package com.ultraguard.buildlogic

import org.gradle.api.Plugin
import org.gradle.api.Project
import org.gradle.kotlin.dsl.dependencies

/**
 * Bir `:feature:*` modulu: Android kutuphanesi + Compose + Hilt + navigation,
 * ustune tasarim sistemi ve domain modelleri. Feature'lar birbirini asla
 * tanimaz; sadece `:core:*` yonunde bagimlilik kurarlar.
 */
class AndroidFeatureConventionPlugin : Plugin<Project> {
    override fun apply(target: Project) = with(target) {
        pluginManager.apply("ultraguard.android.library")
        pluginManager.apply("ultraguard.android.compose")
        pluginManager.apply("ultraguard.android.hilt")

        dependencies {
            add("implementation", project(":core:model"))
            add("implementation", project(":core:common"))
            add("implementation", project(":core:designsystem"))
            add("implementation", libs.findLibrary("hilt-navigation-compose").get())
            add("implementation", libs.findLibrary("androidx-navigation-compose").get())
            add("implementation", libs.findLibrary("androidx-lifecycle-runtime-ktx").get())
        }
    }
}
