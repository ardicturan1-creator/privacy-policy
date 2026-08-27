plugins {
    id("ultraguard.android.library")
    id("ultraguard.android.compose")
}

android { namespace = "com.ultraguard.core.designsystem" }

dependencies {
    api(projects.core.model)
}
