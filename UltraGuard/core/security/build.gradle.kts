plugins {
    id("ultraguard.android.library")
    id("ultraguard.android.hilt")
}

android { namespace = "com.ultraguard.core.security" }

dependencies {
    api(projects.core.model)
    implementation(projects.core.common)
    implementation(libs.androidx.core.ktx)
    implementation(libs.play.integrity)
}
