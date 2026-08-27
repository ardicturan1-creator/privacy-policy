plugins {
    id("ultraguard.android.library")
    id("ultraguard.android.hilt")
}

android {
    namespace = "com.ultraguard.core.ai"

    androidResources {
        // Model dosyalari APK icinde sikistirilmamalidir; MappedByteBuffer ile
        // dogrudan mmap edilirler, aksi halde her acilista RAM'e acilirlar.
        noCompress += listOf("tflite")
    }
}

dependencies {
    api(projects.core.model)
    implementation(projects.core.common)

    implementation(libs.tflite)
    implementation(libs.tflite.support)
}
