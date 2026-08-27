# UltraGuard R8 kurallari

# --- Kendini koruma ---
# Sinif ve metot adlari karistirilmalidir; ancak tersine muhendisligi
# zorlastirmak bir savunma katmanidir, guvenlik sinirinin kendisi degil.
# Gercek sinir hardware-backed keystore ve imza dogrulamasidir.
-repackageclasses
-allowaccessmodification

# --- Room ---
-keep class * extends androidx.room.RoomDatabase { <init>(); }
-keep @androidx.room.Entity class * { *; }

# --- kotlinx.serialization ---
-keepattributes *Annotation*, InnerClasses
-dontnote kotlinx.serialization.**
-keepclassmembers class **$$serializer { *; }
-keepclasseswithmembers class ** {
    kotlinx.serialization.KSerializer serializer(...);
}

# --- TensorFlow Lite ---
-keep class org.tensorflow.lite.** { *; }
-dontwarn org.tensorflow.lite.**

# --- SQLCipher ---
-keep class net.zetetic.database.** { *; }

# --- Hilt ---
-keep class dagger.hilt.** { *; }
-keep class javax.inject.** { *; }

# --- Enum degerleri ---
# EventType ve ThreatClass isimle serilestirildigi icin enum adlari
# korunmalidir; karistirilirlarsa gecmis veri okunamaz hale gelir.
-keepclassmembers enum com.ultraguard.core.model.** {
    public static **[] values();
    public static ** valueOf(java.lang.String);
    public java.lang.String name();
}

# --- WorkManager + Hilt Worker ---
# Worker siniflari yansima ile olusturulur; adlari korunmalidir.
-keep class * extends androidx.work.ListenableWorker { <init>(...); }
-keep @androidx.hilt.work.HiltWorker class * { *; }

# --- Sealed hiyerarsilerin polimorfik serilestirilmesi ---
# EnforcementAction, Action Ledger'da JSON olarak saklanir ve alt tip
# ayirici olarak sinif ADI kullanilir. Karistirilirsa gecmis defter
# kayitlari okunamaz hale gelir -- ve defterin butunlugu, okunabilirligine
# baglidir.
-keep class com.ultraguard.core.model.EnforcementAction { *; }
-keep class com.ultraguard.core.model.EnforcementAction$* { *; }
-keep class com.ultraguard.core.model.Subject { *; }
-keep class com.ultraguard.core.model.Subject$* { *; }
-keep class com.ultraguard.core.model.Attribution { *; }

# --- DeviceAdminReceiver ---
# Manifest'ten adiyla cozulur.
-keep class com.ultraguard.shield.ports.UltraGuardDeviceAdminReceiver { *; }

# --- Servisler ---
# Sistem tarafindan ada gore baglanan servisler karistirilamaz.
-keep class com.ultraguard.core.sensors.UltraGuardAccessibilityService { *; }
-keep class com.ultraguard.core.sensors.NotificationCollector { *; }
-keep class com.ultraguard.core.network.UltraGuardVpnService { *; }
-keep class com.ultraguard.shield.service.ProtectionService { *; }
-keep class com.ultraguard.shield.service.BootReceiver { *; }

# --- Aciklama anahtarlari ---
# `ExplanationText` string kaynaklarini getIdentifier ile ada gore cozer;
# kaynak kucultme bu kaynaklari kullanilmiyor sanip silebilir.
-keepclassmembers class **.R$string {
    public static final int expl_*;
}
