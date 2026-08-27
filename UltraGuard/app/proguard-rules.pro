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
