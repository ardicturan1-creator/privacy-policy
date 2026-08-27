plugins {
    id("ultraguard.android.library")
    id("ultraguard.android.hilt")
    alias(libs.plugins.kotlin.serialization)
}

android { namespace = "com.ultraguard.core.policy" }

dependencies {
    api(projects.core.model)
    implementation(projects.core.common)
    implementation(projects.core.database)
    implementation(projects.core.security)
    implementation(libs.kotlinx.serialization.json)
}
