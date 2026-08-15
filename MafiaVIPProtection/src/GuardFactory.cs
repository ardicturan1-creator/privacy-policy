using System;
using GTA;
using GTA.Math;

namespace MafiaVIP
{
    /// <summary>
    /// Koruma ped'leri ve konvoy araclarinin uretim + yapilandirma merkezi.
    /// Tum spawn islemleri buradan gecer, boylece istatistikler tek yerde.
    /// </summary>
    internal static class GuardFactory
    {
        // ------------------------------------------------------------------
        // Ped uretimi
        // ------------------------------------------------------------------
        /// <summary>
        /// Yapilandirilmis bir koruma ped'i olusturur. Basarisiz olursa null.
        /// </summary>
        public static Ped CreateGuard(Config cfg, Vector3 position, float heading, string weaponOverride = null)
        {
            if (cfg == null) return null;

            try
            {
                Model? model = Utils.RequestAnyModel(cfg.GuardModels);
                if (!model.HasValue) return null;

                Ped ped = World.CreatePed(model.Value, position, heading);
                model.Value.MarkAsNoLongerNeeded();

                if (!Utils.Valid(ped))
                {
                    Logger.Warn("Koruma spawn edilemedi (World.CreatePed null).");
                    return null;
                }

                Configure(ped, cfg, weaponOverride);
                return ped;
            }
            catch (Exception ex)
            {
                Logger.Error("CreateGuard hatasi", ex);
                return null;
            }
        }

        /// <summary>Belirli bir model adiyla ped olusturur (pilot vb. icin).</summary>
        public static Ped CreatePed(Config cfg, string modelName, Vector3 position, float heading, string weaponOverride = null)
        {
            try
            {
                Model? model = Utils.RequestModel(modelName);
                if (!model.HasValue) model = Utils.RequestAnyModel(cfg.GuardModels);
                if (!model.HasValue) return null;

                Ped ped = World.CreatePed(model.Value, position, heading);
                model.Value.MarkAsNoLongerNeeded();
                if (!Utils.Valid(ped)) return null;

                Configure(ped, cfg, weaponOverride);
                return ped;
            }
            catch (Exception ex)
            {
                Logger.Error("CreatePed hatasi: " + modelName, ex);
                return null;
            }
        }

        /// <summary>
        /// Bir ped'i "profesyonel koruma" haline getirir:
        /// saglik, zirh, isabet, savas davranisi, iliski grubu, silahlar, kiyafet.
        /// </summary>
        public static void Configure(Ped ped, Config cfg, string weaponOverride = null)
        {
            if (!Utils.Valid(ped) || cfg == null) return;

            int h = ped.Handle;

            try
            {
                N.SetAsMissionEntity(h);
                ped.IsPersistent = true;
                N.SetKeepTask(h, true);

                // --- Dayaniklilik ---
                N.SetMaxHealth(h, cfg.GuardHealth);
                N.SetHealth(h, cfg.GuardHealth);
                N.SetArmour(h, cfg.GuardArmor);
                N.SetSuffersCriticalHits(h, cfg.GuardCriticalHits);
                N.SetDiesWhenInjured(h, false);
                N.SetCanRagdoll(h, true);
                N.SetCanBeTargetted(h, true);
                if (cfg.GuardInvincible) N.SetInvincible(h, true);

                // --- Savas yetenegi ---
                N.SetAccuracy(h, cfg.GuardAccuracy);
                N.SetShootRate(h, cfg.GuardShootRate);
                N.SetCombatAbility(h, cfg.GuardCombatAbility);
                N.SetCombatRange(h, cfg.GuardCombatRange);
                N.SetCombatMovement(h, cfg.GuardCombatMovement);
                N.SetSeeingRange(h, cfg.GuardSeeingRange);
                N.SetHearingRange(h, cfg.GuardHearingRange);
                N.SetAlertness(h, 3);                 // 3 = tamamen tetikte
                N.SetCanSwitchWeapon(h, true);

                // Savas ozellikleri (BF_*)
                N.SetCombatAttribute(h, 0, true);     // sipere girebilir
                N.SetCombatAttribute(h, 1, true);     // arac kullanabilir
                N.SetCombatAttribute(h, 2, true);     // drive-by yapabilir
                N.SetCombatAttribute(h, 3, true);     // araci terk edebilir
                N.SetCombatAttribute(h, 5, true);     // silahsizken de dovusur
                N.SetCombatAttribute(h, 46, true);    // her zaman dovus (kacma yok)
                N.SetCombatAttribute(h, 17, false);   // asla panikle kacma
                N.SetConfigFlag(h, 32, false);        // on camdan firlamasin
                N.SetCanBeKnockedOffVehicle(h, 1);    // motordan dusmesin

                // --- Iliski grubu ---
                Relationships.Apply(ped);

                // --- Silahlar ---
                GiveWeapons(ped, cfg, weaponOverride);

                // --- Kiyafet ---
                ApplyOutfit(ped, cfg);
            }
            catch (Exception ex)
            {
                Logger.Error("GuardFactory.Configure hatasi", ex);
            }
        }

        public static void GiveWeapons(Ped ped, Config cfg, string weaponOverride = null)
        {
            if (!Utils.Valid(ped) || cfg == null) return;

            try
            {
                string primaryName = !string.IsNullOrEmpty(weaponOverride)
                    ? weaponOverride
                    : Utils.Pick(cfg.GuardWeapons);

                WeaponHash primary = Utils.ResolveWeapon(primaryName);
                ped.Weapons.Give(primary, 1000, true, true);

                if (!string.IsNullOrEmpty(cfg.SidearmWeapon))
                {
                    WeaponHash sidearm = Utils.ResolveWeapon(cfg.SidearmWeapon, WeaponHash.Pistol);
                    ped.Weapons.Give(sidearm, 500, false, true);
                }

                if (cfg.GuardInfiniteAmmo)
                {
                    N.SetInfiniteAmmoClip(ped.Handle, true);
                    N.SetInfiniteAmmo(ped.Handle, true, unchecked((int)primary));
                }

                N.SetCurrentWeapon(ped.Handle, unchecked((int)primary), true);
            }
            catch (Exception ex)
            {
                Logger.Error("GiveWeapons hatasi", ex);
            }
        }

        /// <summary>
        /// Kiyafet uygular. OutfitSets doluysa oradan rastgele bir set,
        /// yoksa (ve RandomOutfits aciksa) oyunun rastgele varyasyonu.
        /// Set formati: "component:drawable:texture|component:drawable:texture"
        /// </summary>
        public static void ApplyOutfit(Ped ped, Config cfg)
        {
            if (!Utils.Valid(ped) || cfg == null) return;

            try
            {
                if (cfg.OutfitSets != null && cfg.OutfitSets.Length > 0)
                {
                    string set = Utils.Pick(cfg.OutfitSets);
                    if (!string.IsNullOrEmpty(set))
                    {
                        foreach (string part in set.Split('|'))
                        {
                            string[] fields = part.Split(':');
                            if (fields.Length < 2) continue;

                            int component, drawable, texture = 0;
                            if (!int.TryParse(fields[0].Trim(), out component)) continue;
                            if (!int.TryParse(fields[1].Trim(), out drawable)) continue;
                            if (fields.Length > 2) int.TryParse(fields[2].Trim(), out texture);

                            N.SetComponent(ped.Handle, component, drawable, texture);
                        }
                        return;
                    }
                }

                if (cfg.RandomOutfits)
                    N.SetRandomOutfit(ped.Handle);
            }
            catch (Exception ex)
            {
                Logger.Error("ApplyOutfit hatasi", ex);
            }
        }

        // ------------------------------------------------------------------
        // Arac uretimi
        // ------------------------------------------------------------------
        public static Vehicle CreateVehicle(Config cfg, string[] models, Vector3 position, float heading)
        {
            if (cfg == null) return null;

            try
            {
                Model? model = Utils.RequestAnyModel(models);
                if (!model.HasValue) return null;

                Vehicle vehicle = World.CreateVehicle(model.Value, position, heading);
                model.Value.MarkAsNoLongerNeeded();

                if (!Utils.Valid(vehicle))
                {
                    Logger.Warn("Arac spawn edilemedi.");
                    return null;
                }

                ConfigureVehicle(vehicle, cfg);
                return vehicle;
            }
            catch (Exception ex)
            {
                Logger.Error("CreateVehicle hatasi", ex);
                return null;
            }
        }

        public static Vehicle CreateVehicle(Config cfg, string model, Vector3 position, float heading)
        {
            return CreateVehicle(cfg, new[] { model }, position, heading);
        }

        public static void ConfigureVehicle(Vehicle vehicle, Config cfg)
        {
            if (!Utils.Valid(vehicle) || cfg == null) return;

            int h = vehicle.Handle;
            try
            {
                N.SetAsMissionEntity(h);
                vehicle.IsPersistent = true;
                N.PlaceOnGroundProperly(h);
                N.SetEngineOn(h, true);
                N.SetDoorsLocked(h, 1);               // 1 = kilitsiz (oyuncu binebilsin)

                if (cfg.ArmoredVehicles)
                {
                    N.SetTyresCanBurst(h, false);
                    N.SetModKit(h, 0);
                    N.SetWindowTint(h, cfg.VehicleTint);
                }

                if (cfg.VehiclePrimaryColor >= 0)
                    N.SetColours(h, cfg.VehiclePrimaryColor, cfg.VehicleSecondaryColor);
            }
            catch (Exception ex)
            {
                Logger.Error("ConfigureVehicle hatasi", ex);
            }
        }

        /// <summary>Suruculuk yeteneklerini ayarlar.</summary>
        public static void ConfigureDriver(Ped ped, ConvoySpeedMode mode)
        {
            if (!Utils.Valid(ped)) return;

            try
            {
                N.SetDriverAbility(ped.Handle, 1f);
                N.SetDriverAggressiveness(ped.Handle, mode == ConvoySpeedMode.Aggressive ? 1f
                                                   : mode == ConvoySpeedMode.Stealth ? 0.2f : 0.55f);
                N.SetCombatAttribute(ped.Handle, 3, false);  // konvoyda kendi kafasina inmesin
            }
            catch (Exception ex)
            {
                Logger.Error("ConfigureDriver hatasi", ex);
            }
        }
    }
}
