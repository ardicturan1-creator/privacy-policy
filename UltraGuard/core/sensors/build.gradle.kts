plugins {
    id("ultraguard.android.library")
    id("ultraguard.android.hilt")
}

android { namespace = "com.ultraguard.core.sensors" }

dependencies {
    api(projects.core.model)
    implementation(projects.core.common)
    implementation(projects.core.engine)
    implementation(libs.androidx.core.ktx)
}
