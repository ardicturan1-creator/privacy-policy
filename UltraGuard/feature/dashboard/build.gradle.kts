plugins {
    id("ultraguard.android.feature")
}

android { namespace = "com.ultraguard.feature.dashboard" }

dependencies {
    implementation(projects.core.policy)
    implementation(projects.core.database)
    implementation(projects.core.datastore)
}
