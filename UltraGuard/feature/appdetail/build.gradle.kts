plugins {
    id("ultraguard.android.feature")
    alias(libs.plugins.kotlin.serialization)
}

android { namespace = "com.ultraguard.feature.appdetail" }

dependencies {
    implementation(projects.core.policy)
    implementation(projects.core.database)
    implementation(libs.kotlinx.serialization.json)
}
