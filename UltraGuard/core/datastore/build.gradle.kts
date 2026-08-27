plugins {
    id("ultraguard.android.library")
    id("ultraguard.android.hilt")
}

android { namespace = "com.ultraguard.core.datastore" }

dependencies {
    api(projects.core.model)
    implementation(projects.core.common)
    implementation(libs.androidx.datastore.preferences)
}
