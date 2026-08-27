package com.ultraguard.core.ai.rules

import com.ultraguard.core.model.Capability
import com.ultraguard.core.model.EventAttributes
import com.ultraguard.core.model.EventType
import com.ultraguard.core.model.RiskScore
import com.ultraguard.core.model.ThreatClass

private const val SECOND = 1_000L
private const val MINUTE = 60 * SECOND
private const val HOUR = 60 * MINUTE
private const val DAY = 24 * HOUR

/**
 * UltraGuard temel kural paketi (`base-v1`).
 *
 * Bu paket uygulamayla birlikte gonderilir ve OTA ile guncellenir.
 * Kurallarin tamami **davranis dizisi** uzerine kuruludur; hicbiri dosya
 * hash'ine veya paket adina bagli degildir. Bu, saldirganin APK'yi yeniden
 * paketlemesini etkisiz kilar.
 */
object RulePack {

    const val VERSION = "base-v1"

    fun all(): List<Rule> = listOf(
        SideloadWithDangerousPermissionTriad,
        AccessibilityGestureOnFreshPackage,
        OverlayOnProtectedScreen,
        BankingWindowQueryWithOverlay,
        ScreenCaptureDuringFinancialApp,
        StalkerwarePersistentSurveillance,
        BackgroundSensorHarvesting,
        CryptoClipperPattern,
        NotificationOtpExfiltration,
        C2BeaconAfterInstall,
        DgaWithBulkUpload,
        PackedDexWithNativeLoader,
        SignatureChangedOnUpdate,
        DebugSurfaceExposed,
        HookingFrameworkPresent,
        FilelessLoaderPattern,
        SensitiveBinderProbing,
    )

    // ---------------------------------------------------------------------
    // Kurulum ve statik triyaj
    // ---------------------------------------------------------------------

    /**
     * R-INST-001 — Bankacilik trojaninin klasik acilis hamlesi.
     *
     * Yan yuklenmis bir paketin erisilebilirlik + ekran ustu cizim + SMS
     * uclusunu birlikte istemesi. Mesru bir uygulamanin bu ucunu ayni anda
     * istemesi icin gecerli bir neden neredeyse yoktur.
     */
    object SideloadWithDangerousPermissionTriad : Rule() {
        override val id = "R-INST-001"
        override val threatClass = ThreatClass.BANKING_OVERLAY_TROJAN
        override val baseScore = RiskScore(78)
        override val interestedIn = setOf(
            EventType.PACKAGE_SIDELOAD_DETECTED,
            EventType.STATIC_DANGEROUS_PERMISSION_SET,
        )

        override fun RuleContext.evaluate(): Boolean {
            val sideloaded = need(EventType.PACKAGE_SIDELOAD_DETECTED, weight = 0.35f)
            val triad = need(EventType.STATIC_DANGEROUS_PERMISSION_SET, weight = 0.45f) { event ->
                event.attr(EventAttributes.MATCHED_PATTERN) == "a11y_overlay_sms"
            }
            if (!sideloaded || !triad) return false

            optional(EventType.STATIC_LEGACY_TARGET_SDK, weight = 0.10f)
            optional(EventType.STATIC_SELF_SIGNED_YOUNG_CERT, weight = 0.10f)
            return true
        }
    }

    /**
     * R-STATIC-011 — Paketlenmis DEX + dinamik yerel kutuphane yukleyici.
     *
     * Yuksek entropi tek basina yanlis pozitif uretir (yasal obfuscation),
     * ancak yerel yukleyiciyle birlestiginde dropper mimarisinin isaretidir.
     */
    object PackedDexWithNativeLoader : Rule() {
        override val id = "R-STATIC-011"
        override val threatClass = ThreatClass.DROPPER
        override val baseScore = RiskScore(62)
        override val interestedIn = setOf(
            EventType.STATIC_HIGH_ENTROPY_DEX,
            EventType.STATIC_NATIVE_LOADER_PRESENT,
        )

        override fun RuleContext.evaluate(): Boolean {
            val packed = need(EventType.STATIC_HIGH_ENTROPY_DEX, weight = 0.5f) { event ->
                (event.attr(EventAttributes.DEX_ENTROPY)?.toFloatOrNull() ?: 0f) >= 7.8f
            }
            val loader = need(EventType.STATIC_NATIVE_LOADER_PRESENT, weight = 0.5f)
            return packed && loader
        }
    }

    /**
     * R-PKG-004 — Guncellemede imzanin degismesi.
     *
     * Android bunu normalde reddeder; gerceklestiyse ya paket kaldirilip
     * yeniden kuruldu ya da imza rotasyonu var. Her iki durumda da tedarik
     * zinciri devralma ihtimali incelenmelidir.
     */
    object SignatureChangedOnUpdate : Rule() {
        override val id = "R-PKG-004"
        override val threatClass = ThreatClass.POLICY_VIOLATION
        override val baseScore = RiskScore(70)
        override val interestedIn = setOf(EventType.PACKAGE_SIGNATURE_CHANGED)

        override fun RuleContext.evaluate(): Boolean =
            need(EventType.PACKAGE_SIGNATURE_CHANGED, weight = 1.0f)
    }

    // ---------------------------------------------------------------------
    // Erisilebilirlik ve ekran ustu saldirilar
    // ---------------------------------------------------------------------

    /**
     * R-A11Y-001 — Yeni kurulan bir paketin jest yetenegi talep etmesi.
     *
     * `canPerformGestures`, uygulamaya kullanici adina dokunma yetkisi verir.
     * Kurulumdan sonraki ilk saatte bunu isteyen bir paket, kullanicinin
     * onaylamadigi islemleri otomatiklestirmeyi hedefliyor demektir.
     */
    object AccessibilityGestureOnFreshPackage : Rule() {
        override val id = "R-A11Y-001"
        override val threatClass = ThreatClass.ACCESSIBILITY_ABUSE
        override val baseScore = RiskScore(85)
        override val interestedIn = setOf(
            EventType.PACKAGE_INSTALLED,
            EventType.ACCESSIBILITY_GESTURE_CAPABILITY,
        )

        override fun RuleContext.evaluate(): Boolean {
            val fresh = needSequence(
                first = EventType.PACKAGE_INSTALLED,
                second = EventType.ACCESSIBILITY_GESTURE_CAPABILITY,
                withinMillis = 1 * HOUR,
                weight = 0.7f,
            )
            if (!fresh) return false
            optional(EventType.PACKAGE_SIDELOAD_DETECTED, weight = 0.3f)

            // Yan yuklenmisse bu artik "supheli" degil, "neredeyse kesin".
            if (window.has(EventType.PACKAGE_SIDELOAD_DETECTED)) escalateTo(RiskScore(93))
            return true
        }
    }

    /**
     * R-A11Y-002 — Banka penceresi sorgulama + ekran ustu cizim.
     *
     * Overlay saldirisinin calisma anindaki imzasi: sahte giris ekranini
     * gercek bankacilik uygulamasinin uzerine bindirme.
     */
    object BankingWindowQueryWithOverlay : Rule() {
        override val id = "R-A11Y-002"
        override val threatClass = ThreatClass.BANKING_OVERLAY_TROJAN
        override val baseScore = RiskScore(92)
        override val interestedIn = setOf(
            EventType.ACCESSIBILITY_WINDOW_QUERY,
            EventType.OVERLAY_DRAWN,
        )

        override fun RuleContext.evaluate(): Boolean {
            val queriedFinancialWindow = need(EventType.ACCESSIBILITY_WINDOW_QUERY, weight = 0.5f) { event ->
                event.attr(EventAttributes.PROTECTED_CATEGORY) == "financial"
            }
            val drew = need(EventType.OVERLAY_DRAWN, weight = 0.5f)
            if (!queriedFinancialWindow || !drew) return false

            optional(EventType.NETWORK_BULK_UPLOAD, weight = 0.2f)
            return true
        }
    }

    /**
     * R-UI-003 — Korunan bir ekran acikken ekran ustune cizim.
     *
     * Finansal Kalkan aktifken herhangi bir ucuncu taraf overlay'i, niyeti
     * ne olursa olsun engellenir. Tapjacking'e karsi en dogrudan savunma.
     */
    object OverlayOnProtectedScreen : Rule() {
        override val id = "R-UI-003"
        override val threatClass = ThreatClass.CREDENTIAL_PHISHING
        override val baseScore = RiskScore(80)
        override val interestedIn = setOf(EventType.OVERLAY_ON_PROTECTED_SCREEN)

        override fun RuleContext.evaluate(): Boolean =
            need(EventType.OVERLAY_ON_PROTECTED_SCREEN, weight = 1.0f)
    }

    /**
     * R-FIN-001 — Finansal uygulama on plandayken ekran yakalama.
     *
     * MediaProjection mesru ekran kaydi araclarinda da vardir; ayirt edici
     * olan, tam da bankacilik oturumu sirasinda baslatilmis olmasidir.
     */
    object ScreenCaptureDuringFinancialApp : Rule() {
        override val id = "R-FIN-001"
        override val threatClass = ThreatClass.CREDENTIAL_PHISHING
        override val baseScore = RiskScore(88)
        override val interestedIn = setOf(EventType.MEDIA_PROJECTION_STARTED)

        override fun RuleContext.evaluate(): Boolean =
            need(EventType.MEDIA_PROJECTION_STARTED, weight = 1.0f) { event ->
                event.attr(EventAttributes.PROTECTED_CATEGORY) == "financial"
            }
    }

    // ---------------------------------------------------------------------
    // Casusluk ve veri hasadi
    // ---------------------------------------------------------------------

    /**
     * R-STALK-001 — Ticari stalkerware davranis profili.
     *
     * Imza gerektirmez: gizli calisan, surekli konum alan ve bunu duzenli
     * araliklarla disari gonderen herhangi bir paket bu kalibi doldurur.
     * Stalkerware'in yeniden paketlenmesi bu kurali atlatmaya yetmez.
     */
    object StalkerwarePersistentSurveillance : Rule() {
        override val id = "R-STALK-001"
        override val threatClass = ThreatClass.STALKERWARE
        override val baseScore = RiskScore(90)
        override val interestedIn = setOf(
            EventType.SENSOR_LOCATION_ACCESS,
            EventType.SENSOR_BACKGROUND_ACCESS,
            EventType.NETWORK_BEACON_PATTERN,
        )

        override fun RuleContext.evaluate(): Boolean {
            val recent = window.recent(6 * HOUR)
            // Alti saatte en az 12 arka plan konum erisimi: dakikalik takip.
            val persistent = recent.ofType(EventType.SENSOR_LOCATION_ACCESS)
                .count { it.attrBool(EventAttributes.FOREGROUND).not() } >= 12
            if (!persistent) return false

            val located = need(EventType.SENSOR_LOCATION_ACCESS, weight = 0.4f) {
                !it.attrBool(EventAttributes.FOREGROUND)
            }
            val beacons = need(EventType.NETWORK_BEACON_PATTERN, weight = 0.4f)
            if (!located || !beacons) return false

            optional(EventType.SENSOR_MICROPHONE_ACCESS, weight = 0.2f)
            return true
        }
    }

    /**
     * R-SPY-002 — Ekran kapaliyken kamera veya mikrofona erisim.
     *
     * Mesru bir kullanim senaryosu yok denecek kadar azdir (ses kaydedici
     * uygulamalar kullanici tarafindan aciktan baslatilir ve on plandadir).
     */
    object BackgroundSensorHarvesting : Rule() {
        override val id = "R-SPY-002"
        override val threatClass = ThreatClass.SPYWARE_GENERIC
        override val baseScore = RiskScore(84)
        override val interestedIn = setOf(
            EventType.SENSOR_CAMERA_ACCESS,
            EventType.SENSOR_MICROPHONE_ACCESS,
        )

        override fun RuleContext.evaluate(): Boolean {
            val covert = needAnyOf(
                weight = 0.8f,
                EventType.SENSOR_CAMERA_ACCESS,
                EventType.SENSOR_MICROPHONE_ACCESS,
            )
            if (!covert) return false

            val screenOff = window.events.any {
                it.type in interestedIn &&
                    !it.attrBool(EventAttributes.SCREEN_ON) &&
                    !it.attrBool(EventAttributes.FOREGROUND)
            }
            if (!screenOff) return false

            optional(EventType.NETWORK_BULK_UPLOAD, weight = 0.2f)
            return true
        }
    }

    /**
     * R-CLIP-001 — Kripto adresi degistirme (clipper).
     *
     * Panoda cuzdan adresi tespit edilmesinin hemen ardindan ayni paketin
     * panoya yazmasi. Saldiri, kullanicinin fark etmeyecegi kadar hizlidir;
     * tespit penceresi bu yuzden 5 saniyedir.
     */
    object CryptoClipperPattern : Rule() {
        override val id = "R-CLIP-001"
        override val threatClass = ThreatClass.CRYPTO_CLIPPER
        override val baseScore = RiskScore(91)
        override val interestedIn = setOf(
            EventType.CLIPBOARD_SENSITIVE_CONTENT,
            EventType.CLIPBOARD_READ,
        )

        override fun RuleContext.evaluate(): Boolean =
            need(EventType.CLIPBOARD_SENSITIVE_CONTENT, weight = 0.6f) { event ->
                event.attr(EventAttributes.MATCHED_PATTERN) in setOf("crypto_address", "iban")
            } && need(EventType.CLIPBOARD_READ, weight = 0.4f) { read ->
                val sensitive = window.latest(EventType.CLIPBOARD_SENSITIVE_CONTENT)
                sensitive != null && read.timestampMillis - sensitive.timestampMillis in 0..(5 * SECOND)
            }
    }

    /**
     * R-OTP-001 — Tek kullanimlik sifre sizdirma.
     *
     * Bildirimden OTP okunmasinin ardindan kisa surede disari veri gonderimi.
     * Bildirim **icerigi** hicbir zaman kaydedilmez; yalnizca "OTP kalibi
     * eslesti" turetilmis sinyali olay olarak yayilir.
     */
    object NotificationOtpExfiltration : Rule() {
        override val id = "R-OTP-001"
        override val threatClass = ThreatClass.SMS_FRAUD
        override val baseScore = RiskScore(89)
        override val interestedIn = setOf(
            EventType.NOTIFICATION_PHISHING_PATTERN,
            EventType.NETWORK_BULK_UPLOAD,
        )

        override fun RuleContext.evaluate(): Boolean =
            needSequence(
                first = EventType.NOTIFICATION_PHISHING_PATTERN,
                second = EventType.NETWORK_BULK_UPLOAD,
                withinMillis = 30 * SECOND,
                weight = 1.0f,
            )
    }

    // ---------------------------------------------------------------------
    // Ag ve komuta-kontrol
    // ---------------------------------------------------------------------

    /**
     * R-NET-001 — Kurulumdan kisa sure sonra baslayan duzenli beacon.
     *
     * Sabit araliklarla kucuk paketler = C2 kalp atisi. Sifre cozmeden,
     * yalnizca paket boyutu ve zamanlama histogramindan tespit edilir.
     */
    object C2BeaconAfterInstall : Rule() {
        override val id = "R-NET-001"
        override val threatClass = ThreatClass.C2_BEACON
        override val baseScore = RiskScore(76)
        override val interestedIn = setOf(
            EventType.NETWORK_BEACON_PATTERN,
            EventType.PACKAGE_INSTALLED,
        )

        override fun RuleContext.evaluate(): Boolean {
            val beaconing = needSequence(
                first = EventType.PACKAGE_INSTALLED,
                second = EventType.NETWORK_BEACON_PATTERN,
                withinMillis = 24 * HOUR,
                weight = 0.7f,
            )
            if (!beaconing) return false

            optional(EventType.NETWORK_REPUTATION_HIT, weight = 0.3f)
            if (window.has(EventType.NETWORK_REPUTATION_HIT)) escalateTo(RiskScore(90))
            return true
        }
    }

    /**
     * R-NET-004 — Algoritma uretimi alan adi + toplu veri gonderimi.
     *
     * DGA, C2 altyapisinin kapatilmasina karsi dayaniklilik saglar; yuksek
     * entropili alan adina yapilan buyuk yuklemeler sizdirmanin kendisidir.
     */
    object DgaWithBulkUpload : Rule() {
        override val id = "R-NET-004"
        override val threatClass = ThreatClass.DATA_EXFILTRATION
        override val baseScore = RiskScore(87)
        override val interestedIn = setOf(
            EventType.NETWORK_DGA_DOMAIN,
            EventType.NETWORK_BULK_UPLOAD,
        )

        override fun RuleContext.evaluate(): Boolean {
            val dga = need(EventType.NETWORK_DGA_DOMAIN, weight = 0.5f) { event ->
                (event.attr(EventAttributes.DOMAIN_ENTROPY)?.toFloatOrNull() ?: 0f) >= 3.6f
            }
            val upload = need(EventType.NETWORK_BULK_UPLOAD, weight = 0.5f) { event ->
                (event.attrLong(EventAttributes.BYTES_OUT) ?: 0L) >= 256 * 1024
            }
            return dga && upload
        }
    }

    // ---------------------------------------------------------------------
    // Cihaz butunlugu ve kendini koruma
    // ---------------------------------------------------------------------

    /**
     * R-SYS-002 — Hata ayiklama yuzeyi acik.
     *
     * ADB veya kablosuz hata ayiklama acik bir cihazda, fiziksel erisimi
     * olan biri kilit ekranini asmadan veri cekebilir. Dusuk skorlu ama
     * her zaman gorunur bir yapilandirma bulgusu.
     */
    object DebugSurfaceExposed : Rule() {
        override val id = "R-SYS-002"
        override val threatClass = ThreatClass.POLICY_VIOLATION
        override val baseScore = RiskScore(45)
        override val interestedIn = setOf(
            EventType.ADB_ENABLED,
            EventType.WIRELESS_DEBUGGING_ENABLED,
        )

        override fun RuleContext.evaluate(): Boolean {
            val exposed = needAnyOf(
                weight = 1.0f,
                EventType.WIRELESS_DEBUGGING_ENABLED,
                EventType.ADB_ENABLED,
            )
            if (!exposed) return false
            // Kablosuz hata ayiklama fiziksel erisim bile gerektirmez.
            if (window.has(EventType.WIRELESS_DEBUGGING_ENABLED)) escalateTo(RiskScore(60))
            return true
        }
    }

    /**
     * R-SELF-001 — Hooking cercevesi tespiti (Frida, Xposed, LSPosed).
     *
     * UltraGuard'in kendisini hedef alan bir saldirinin ilk adimi. Tespit
     * edilirse Kasa kilitlenir ve kritik yaptirim yollari devre disi kalir —
     * kandirilmis bir motorun karar vermesindense hic karar vermemesi yegdir.
     */
    object HookingFrameworkPresent : Rule() {
        override val id = "R-SELF-001"
        override val threatClass = ThreatClass.ROOT_EXPLOIT_ATTEMPT
        override val baseScore = RiskScore(82)
        override val interestedIn = setOf(
            EventType.HOOKING_FRAMEWORK_DETECTED,
            EventType.SELF_TAMPER_SUSPECTED,
        )

        override fun RuleContext.evaluate(): Boolean {
            val hooked = needAnyOf(
                weight = 1.0f,
                EventType.HOOKING_FRAMEWORK_DETECTED,
                EventType.SELF_TAMPER_SUSPECTED,
            )
            return hooked
        }
    }

    // ---------------------------------------------------------------------
    // Yalnizca root/KernelSU cihazlarda degerlendirilen kurallar [R]
    // ---------------------------------------------------------------------

    /**
     * R-KRN-001 — Dosyasiz yukleyici.
     *
     * `memfd_create` ile bellekte olusturulan bir imajin hemen ardindan
     * `execve` edilmesi, diske hicbir sey yazmayan bir payload demektir.
     * Klasik dosya taramasinin yapisal olarak goremedigi tek sey budur.
     */
    object FilelessLoaderPattern : Rule() {
        override val id = "R-KRN-001"
        override val threatClass = ThreatClass.FILELESS_LOADER
        override val baseScore = RiskScore(94)
        override val requiredCapability = Capability.ROOTED
        override val interestedIn = setOf(
            EventType.KERNEL_MEMFD_CREATE,
            EventType.KERNEL_EXEC,
        )

        override fun RuleContext.evaluate(): Boolean =
            needSequence(
                first = EventType.KERNEL_MEMFD_CREATE,
                second = EventType.KERNEL_EXEC,
                withinMillis = 2 * SECOND,
                weight = 1.0f,
            )
    }

    /**
     * R-KRN-004 — Hassas Binder islemleri uzerinden izin atlatma yoklamasi.
     *
     * Bir uygulamanin sahip olmadigi izinlerin arkasindaki sistem
     * servislerine tekrar tekrar istek gondermesi, bilinen bir IPC
     * zafiyetini aramaktan baska bir sey degildir.
     */
    object SensitiveBinderProbing : Rule() {
        override val id = "R-KRN-004"
        override val threatClass = ThreatClass.ROOT_EXPLOIT_ATTEMPT
        override val baseScore = RiskScore(86)
        override val requiredCapability = Capability.ROOTED
        override val interestedIn = setOf(EventType.BINDER_SENSITIVE_TRANSACTION)

        override fun RuleContext.evaluate(): Boolean {
            val probes = window.recent(5 * MINUTE).count(EventType.BINDER_SENSITIVE_TRANSACTION)
            if (probes < 20) return false
            return need(EventType.BINDER_SENSITIVE_TRANSACTION, weight = 1.0f)
        }
    }
}
