plugins {
    id("ultraguard.android.feature")
}

android { namespace = "com.ultraguard.feature.settings" }

dependencies {
    implementation(projects.core.policy)
    implementation(projects.core.datastore)
}
