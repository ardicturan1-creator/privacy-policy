package com.ultraguard.core.ai.rules

import com.google.common.truth.Truth.assertThat
import com.ultraguard.core.model.Capability
import com.ultraguard.core.model.DecisionTier
import com.ultraguard.core.model.EventAttributes
import com.ultraguard.core.model.EventType
import com.ultraguard.core.model.RiskBand
import com.ultraguard.core.model.ThreatClass
import org.junit.Test

class RuleEngineTest {

    private val unrootedEngine = RuleEngine(
        rules = RulePack.all(),
        availableCapabilities = setOf(Capability.UNROOTED),
    )

    private val rootedEngine = RuleEngine(
        rules = RulePack.all(),
        availableCapabilities = setOf(Capability.UNROOTED, Capability.ROOTED),
    )

    @Test
    fun `bos pencere hukum uretmez`() {
        val verdict = unrootedEngine.evaluate(windowOf(), T0)
        assertThat(verdict).isNull()
    }

    @Test
    fun `tek basina kurulum olayi hicbir kurali tetiklemez`() {
        val window = windowOf(event(EventType.PACKAGE_INSTALLED, T0))
        assertThat(unrootedEngine.evaluate(window, T0)).isNull()
    }

    @Test
    fun `yan yukleme ve izin ucluşu bankacilik trojani olarak siniflanir`() {
        val window = windowOf(
            event(EventType.PACKAGE_INSTALLED, T0),
            event(
                EventType.PACKAGE_SIDELOAD_DETECTED, T0 + 100,
                attributes = arrayOf(EventAttributes.INSTALLER_PACKAGE to "com.android.chrome"),
            ),
            event(
                EventType.STATIC_DANGEROUS_PERMISSION_SET, T0 + 4_000,
                attributes = arrayOf(EventAttributes.MATCHED_PATTERN to "a11y_overlay_sms"),
            ),
        )

        val verdict = requireNotNull(unrootedEngine.evaluate(window, T0 + 5_000))

        assertThat(verdict.threatClass).isEqualTo(ThreatClass.BANKING_OVERLAY_TROJAN)
        assertThat(verdict.originId).isEqualTo("R-INST-001")
        assertThat(verdict.score.band).isAnyOf(RiskBand.HIGH, RiskBand.CRITICAL)
        assertThat(verdict.tier).isEqualTo(DecisionTier.RULE_ENGINE)
    }

    @Test
    fun `farkli izin kalibi ucluşu kurali tetiklemez`() {
        val window = windowOf(
            event(EventType.PACKAGE_SIDELOAD_DETECTED, T0),
            event(
                EventType.STATIC_DANGEROUS_PERMISSION_SET, T0 + 1_000,
                attributes = arrayOf(EventAttributes.MATCHED_PATTERN to "storage_only"),
            ),
        )
        val verdict = unrootedEngine.evaluate(window, T0 + 2_000)
        assertThat(verdict?.originId).isNotEqualTo("R-INST-001")
    }

    @Test
    fun `her hukum en az bir kanit tasir`() {
        val window = windowOf(
            event(EventType.PACKAGE_SIDELOAD_DETECTED, T0),
            event(
                EventType.STATIC_DANGEROUS_PERMISSION_SET, T0 + 1_000,
                attributes = arrayOf(EventAttributes.MATCHED_PATTERN to "a11y_overlay_sms"),
            ),
        )
        val verdict = requireNotNull(unrootedEngine.evaluate(window, T0 + 2_000))
        assertThat(verdict.attributions).isNotEmpty()
    }

    @Test
    fun `attribution agirliklari bire normalize edilir`() {
        val window = windowOf(
            event(EventType.PACKAGE_SIDELOAD_DETECTED, T0),
            event(
                EventType.STATIC_DANGEROUS_PERMISSION_SET, T0 + 1_000,
                attributes = arrayOf(EventAttributes.MATCHED_PATTERN to "a11y_overlay_sms"),
            ),
            event(EventType.STATIC_LEGACY_TARGET_SDK, T0 + 1_100),
            event(EventType.STATIC_SELF_SIGNED_YOUNG_CERT, T0 + 1_200),
        )
        val verdict = requireNotNull(unrootedEngine.evaluate(window, T0 + 2_000))
        val sum = verdict.attributions.map { it.weight }.sum()
        assertThat(sum).isWithin(0.001f).of(1.0f)
    }

    @Test
    fun `jest yetenegi yan yukleme ile birleşince skor yukselir`() {
        val withoutSideload = windowOf(
            event(EventType.PACKAGE_INSTALLED, T0),
            event(EventType.ACCESSIBILITY_GESTURE_CAPABILITY, T0 + 12_000),
        )
        val withSideload = windowOf(
            event(EventType.PACKAGE_INSTALLED, T0),
            event(EventType.PACKAGE_SIDELOAD_DETECTED, T0 + 100),
            event(EventType.ACCESSIBILITY_GESTURE_CAPABILITY, T0 + 12_000),
        )

        val plain = requireNotNull(unrootedEngine.evaluate(withoutSideload, T0 + 20_000))
        val escalated = requireNotNull(unrootedEngine.evaluate(withSideload, T0 + 20_000))

        assertThat(escalated.score.value).isGreaterThan(plain.score.value)
    }

    @Test
    fun `jest yetenegi bir saatten sonra gelirse taze paket kurali eslesmez`() {
        val window = windowOf(
            event(EventType.PACKAGE_INSTALLED, T0),
            event(EventType.ACCESSIBILITY_GESTURE_CAPABILITY, T0 + 3 * 3_600_000L),
            nowMillis = T0 + 4 * 3_600_000L,
        )
        val verdict = unrootedEngine.evaluate(window, T0 + 4 * 3_600_000L)
        assertThat(verdict?.originId).isNotEqualTo("R-A11Y-001")
    }

    @Test
    fun `kripto clipper yalnizca hizli pano yazimi ile tetiklenir`() {
        val fast = windowOf(
            event(
                EventType.CLIPBOARD_SENSITIVE_CONTENT, T0,
                attributes = arrayOf(EventAttributes.MATCHED_PATTERN to "crypto_address"),
            ),
            event(EventType.CLIPBOARD_READ, T0 + 900),
        )
        val slow = windowOf(
            event(
                EventType.CLIPBOARD_SENSITIVE_CONTENT, T0,
                attributes = arrayOf(EventAttributes.MATCHED_PATTERN to "crypto_address"),
            ),
            event(EventType.CLIPBOARD_READ, T0 + 60_000),
        )

        assertThat(unrootedEngine.evaluate(fast, T0 + 2_000)?.threatClass)
            .isEqualTo(ThreatClass.CRYPTO_CLIPPER)
        assertThat(unrootedEngine.evaluate(slow, T0 + 70_000)?.threatClass)
            .isNotEqualTo(ThreatClass.CRYPTO_CLIPPER)
    }

    @Test
    fun `root gerektiren kurallar rootsuz cihazda hic degerlendirilmez`() {
        val window = windowOf(
            event(EventType.KERNEL_MEMFD_CREATE, T0),
            event(EventType.KERNEL_EXEC, T0 + 500),
        )

        assertThat(unrootedEngine.evaluate(window, T0 + 1_000)).isNull()

        val rootedVerdict = requireNotNull(rootedEngine.evaluate(window, T0 + 1_000))
        assertThat(rootedVerdict.threatClass).isEqualTo(ThreatClass.FILELESS_LOADER)
    }

    @Test
    fun `rootsuz motor root kurallarini kural sayisina dahil etmez`() {
        assertThat(unrootedEngine.ruleCount()).isLessThan(rootedEngine.ruleCount())
    }

    @Test
    fun `birden fazla kural tetiklendiginde skor destekle yukselir`() {
        val single = windowOf(
            event(EventType.PACKAGE_SIDELOAD_DETECTED, T0),
            event(
                EventType.STATIC_DANGEROUS_PERMISSION_SET, T0 + 1_000,
                attributes = arrayOf(EventAttributes.MATCHED_PATTERN to "a11y_overlay_sms"),
            ),
        )
        val corroborated = windowOf(
            event(EventType.PACKAGE_SIDELOAD_DETECTED, T0),
            event(
                EventType.STATIC_DANGEROUS_PERMISSION_SET, T0 + 1_000,
                attributes = arrayOf(EventAttributes.MATCHED_PATTERN to "a11y_overlay_sms"),
            ),
            event(
                EventType.STATIC_HIGH_ENTROPY_DEX, T0 + 1_100,
                attributes = arrayOf(EventAttributes.DEX_ENTROPY to "7.94"),
            ),
            event(EventType.STATIC_NATIVE_LOADER_PRESENT, T0 + 1_200),
        )

        val a = requireNotNull(unrootedEngine.evaluate(single, T0 + 2_000))
        val b = requireNotNull(unrootedEngine.evaluate(corroborated, T0 + 2_000))
        assertThat(b.score.value).isGreaterThan(a.score.value)
    }

    @Test
    fun `dusuk entropili dex paketlenmis kabul edilmez`() {
        val window = windowOf(
            event(
                EventType.STATIC_HIGH_ENTROPY_DEX, T0,
                attributes = arrayOf(EventAttributes.DEX_ENTROPY to "6.10"),
            ),
            event(EventType.STATIC_NATIVE_LOADER_PRESENT, T0 + 100),
        )
        assertThat(unrootedEngine.evaluate(window, T0 + 1_000)).isNull()
    }

    @Test
    fun `skor hicbir zaman yuzu asmaz`() {
        val events = buildList {
            add(event(EventType.PACKAGE_INSTALLED, T0))
            add(event(EventType.PACKAGE_SIDELOAD_DETECTED, T0 + 10))
            add(
                event(
                    EventType.STATIC_DANGEROUS_PERMISSION_SET, T0 + 20,
                    attributes = arrayOf(EventAttributes.MATCHED_PATTERN to "a11y_overlay_sms"),
                ),
            )
            add(event(EventType.ACCESSIBILITY_GESTURE_CAPABILITY, T0 + 30))
            add(
                event(
                    EventType.ACCESSIBILITY_WINDOW_QUERY, T0 + 40,
                    attributes = arrayOf(EventAttributes.PROTECTED_CATEGORY to "financial"),
                ),
            )
            add(event(EventType.OVERLAY_DRAWN, T0 + 50))
            add(event(EventType.OVERLAY_ON_PROTECTED_SCREEN, T0 + 60))
            add(
                event(
                    EventType.NETWORK_DGA_DOMAIN, T0 + 70,
                    attributes = arrayOf(EventAttributes.DOMAIN_ENTROPY to "4.2"),
                ),
            )
            add(
                event(
                    EventType.NETWORK_BULK_UPLOAD, T0 + 80,
                    attributes = arrayOf(EventAttributes.BYTES_OUT to "1048576"),
                ),
            )
        }

        val verdict = requireNotNull(unrootedEngine.evaluate(windowOf(*events.toTypedArray()), T0 + 1_000))
        assertThat(verdict.score.value).isAtMost(100)
    }
}
