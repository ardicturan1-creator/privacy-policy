package com.ultraguard.buildlogic

import org.gradle.api.Plugin
import org.gradle.api.Project
import org.gradle.api.plugins.JavaPluginExtension
import org.gradle.kotlin.dsl.configure
import org.gradle.kotlin.dsl.dependencies
import org.jetbrains.kotlin.gradle.dsl.KotlinJvmProjectExtension

/**
 * Saf Kotlin/JVM modulleri icin. `:core:model` ve kural motorunun saf
 * bolumleri Android'e bagimli degildir; boylece JVM birim testleriyle
 * emulator olmadan, milisaniyeler icinde dogrulanabilirler.
 */
class JvmLibraryConventionPlugin : Plugin<Project> {
    override fun apply(target: Project) = with(target) {
        pluginManager.apply("org.jetbrains.kotlin.jvm")

        extensions.configure<JavaPluginExtension> {
            sourceCompatibility = UltraGuardConfig.JAVA_VERSION
            targetCompatibility = UltraGuardConfig.JAVA_VERSION
        }

        extensions.configure<KotlinJvmProjectExtension> {
            compilerOptions.jvmTarget.set(UltraGuardConfig.JVM_TARGET)
        }

        dependencies {
            add("implementation", libs.findLibrary("kotlinx-coroutines-core").get())
            add("testImplementation", libs.findLibrary("junit").get())
            add("testImplementation", libs.findLibrary("truth").get())
            add("testImplementation", libs.findLibrary("turbine").get())
            add("testImplementation", libs.findLibrary("kotlinx-coroutines-test").get())
        }
    }
}
