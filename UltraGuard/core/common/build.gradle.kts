plugins {
    id("ultraguard.android.library")
    id("ultraguard.android.hilt")
}

android { namespace = "com.ultraguard.core.common" }

dependencies {
    api(projects.core.model)
    implementation(libs.androidx.core.ktx)
}
