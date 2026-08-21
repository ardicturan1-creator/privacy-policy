using System;
using System.IO;
using System.Text;
using System.Windows.Forms;

namespace MafiaVIP
{
    /// <summary>
    /// Tum mod ayarlari. MafiaVIPProtection.ini dosyasindan okunur.
    /// Dosya yoksa varsayilan degerlerle otomatik olusturulur.
    /// </summary>
    public sealed class Config
    {
        // ---------------- Tuslar ----------------
        public Keys MenuKey = Keys.F7;
        public Keys ModifierKey = Keys.None;          // Opsiyonel: hizli tuslar icin gerekli modifier
        public Keys GuardsToggleKey = Keys.F8;
        public Keys ConvoyToggleKey = Keys.F9;
        public Keys AirSupportToggleKey = Keys.F10;
        public Keys BackupKey = Keys.F11;
        public Keys FormationCycleKey = Keys.None;
        public Keys ClearAllKey = Keys.None;

        // ---------------- Genel ----------------
        public bool EnableBlips = true;
        public bool ShowNotifications = true;
        public bool AutoStartOnLoad = false;
        public float ThreatRadius = 65f;
        public bool EngagePolice = false;             // Polisi tehdit say (aranma varsa)
        public bool RemoveDeadBodies = true;
        public int DeadBodyCleanupDelay = 25000;
        public DeathBehavior PlayerDeath = DeathBehavior.Guard;
        public float TeleportDistance = 130f;         // Bu mesafeden uzaktaki koruma isinlanir
        public string RelationshipGroupName = "MAFIA_VIP_GUARDS";
        public bool CodeRedEnabled = true;             // Ani can kaybinda tum ekip gecici olarak agresiflesir
        public int CodeRedHealthDropThreshold = 35;    // Bu kadar ani can dususu Kod Kirmizi tetikler
        public int CodeRedDuration = 12000;            // ms, agresif mod ne kadar surer
        public float StuckSpeedThreshold = 1.5f;       // Bu hizin altinda "sikisti" sayilmaya baslar
        public int StuckTimeThreshold = 7000;          // ms, bu sure sikisik kalirsa kurtarma devreye girer

        // ---------------- Yakin koruma ----------------
        public int GuardCount = 6;
        public int MaxGuards = 8;
        public string[] GuardModels =
        {
            "s_m_m_highsec_01", "s_m_m_highsec_02", "s_m_y_blackops_01",
            "s_m_m_armoured_01", "s_m_m_armoured_02"
        };
        public string[] GuardWeapons =
        {
            "CarbineRifle", "SpecialCarbine", "CombatMG", "PumpShotgun"
        };
        public string SidearmWeapon = "Pistol";
        public int GuardHealth = 400;
        public int GuardArmor = 100;
        public int GuardAccuracy = 75;
        public int GuardShootRate = 800;
        public int GuardCombatAbility = 2;            // 0 zayif, 1 orta, 2 profesyonel
        public int GuardCombatRange = 2;              // 0 yakin, 1 orta, 2 uzak
        public int GuardCombatMovement = 2;           // 0 sabit, 1 defansif, 2 ilerleyen
        public float GuardSeeingRange = 90f;
        public float GuardHearingRange = 110f;
        public bool GuardInvincible = false;
        public bool GuardCriticalHits = false;        // false = kafa vurusu aninda oldurmez
        public bool GuardInfiniteAmmo = true;
        public bool GuardAutoRespawn = true;
        public int GuardRespawnDelay = 15000;
        public float GuardFollowSpeed = 2f;           // 1 = yuru, 2 = kos, 3 = sprint
        public float GuardStoppingRange = 1.2f;
        public Formation DefaultFormation = Formation.Diamond;
        public GuardBehavior DefaultBehavior = GuardBehavior.Defensive;
        public bool RandomOutfits = true;
        public string[] OutfitSets = new string[0];   // "3:1:0|4:1:0|8:1:0|11:1:0" formatinda
        public int GuardBlipColor = 2;                // 2 = yesil
        public int GuardBlipSprite = 1;

        // ---------------- Konvoy ----------------
        public bool SpawnVipVehicle = true;           // false ise oyuncunun mevcut araci VIP kabul edilir
        public bool WarpPlayerIntoVip = false;
        public string VipVehicleModel = "cognoscenti2";
        public string[] LeadVehicleModels = { "kuruma2", "schafter6" };
        public string[] SupportVehicleModels = { "baller6", "insurgent3", "kuruma2" };
        public int LeadVehicleCount = 1;
        public int SupportVehicleCount = 2;
        public int GuardsPerVehicle = 3;
        public float SpeedNormal = 22f;
        public float SpeedAggressive = 38f;
        public float SpeedStealth = 14f;
        public int DrivingStyleNormal = DrivingStylePreset.Normal;
        public int DrivingStyleAggressive = DrivingStylePreset.Rushed;
        public int DrivingStyleStealth = DrivingStylePreset.Normal;
        public float ConvoyMinDistance = 12f;
        public float ConvoyNoRoadsDistance = 20f;      // Bu mesafenin altinda arac yol agini yok sayip dogrudan takip eder
        public bool DeployOnStop = true;
        public float DeployRadius = 6.5f;
        public bool ArmoredVehicles = true;           // Lastik patlamaz + cam filmi
        public int VehicleTint = 5;
        public int VehiclePrimaryColor = 0;           // 0 = siyah
        public int VehicleSecondaryColor = 0;
        public int ConvoyBlipColor = 3;               // 3 = mavi
        public int ConvoyBlipSprite = 225;

        // ---------------- Hava destegi ----------------
        public string[] AirVehicleModels = { "buzzard", "savage", "valkyrie", "annihilator" };
        public string DefaultAirVehicle = "buzzard";
        public string PilotModel = "s_m_y_blackops_01";
        public int GunnerCount = 2;
        public int MaxAirUnits = 3;                    // Ayni anda cagrilabilecek hava birimi sayisi
        public float AirHeight = 45f;
        public float AirRadius = 60f;
        public float AirSpeed = 40f;
        public bool AirAutoEngage = true;
        public bool AirInvincible = false;
        public string CargobobModel = "cargobob3";
        public int LandingTimeout = 45000;             // ms, bu sureden sonra manuel inis devreye girer
        public int AirBlipColor = 5;                  // 5 = sari
        public int AirBlipSprite = 43;

        // ---------------- Yedek (backup) ----------------
        public string[] BackupVehicleModels = { "insurgent3", "baller6", "kuruma2" };
        public int BackupSquadSize = 4;
        public float BackupSpawnDistance = 160f;
        public int BackupCooldown = 45000;

        // ==================================================================
        // Yukleme
        // ==================================================================
        public static string IniPath
        {
            get { return Path.Combine(AppDomain.CurrentDomain.BaseDirectory, "scripts", "MafiaVIPProtection.ini"); }
        }

        public static Config Load()
        {
            Config c = new Config();

            try
            {
                string path = IniPath;

                if (!File.Exists(path))
                {
                    // Ilk calistirmada varsayilan ini'yi olustur.
                    c.WriteDefaultIni(path);
                    return c;
                }

                IniFile ini = new IniFile();
                ini.Load(path);
                if (!ini.Loaded) return c;

                // -------- Keys --------
                c.MenuKey = ini.GetKey("Keys", "MenuKey", c.MenuKey);
                c.ModifierKey = ini.GetKey("Keys", "ModifierKey", c.ModifierKey);
                c.GuardsToggleKey = ini.GetKey("Keys", "GuardsToggleKey", c.GuardsToggleKey);
                c.ConvoyToggleKey = ini.GetKey("Keys", "ConvoyToggleKey", c.ConvoyToggleKey);
                c.AirSupportToggleKey = ini.GetKey("Keys", "AirSupportToggleKey", c.AirSupportToggleKey);
                c.BackupKey = ini.GetKey("Keys", "BackupKey", c.BackupKey);
                c.FormationCycleKey = ini.GetKey("Keys", "FormationCycleKey", c.FormationCycleKey);
                c.ClearAllKey = ini.GetKey("Keys", "ClearAllKey", c.ClearAllKey);

                // -------- General --------
                c.EnableBlips = ini.GetBool("General", "EnableBlips", c.EnableBlips);
                c.ShowNotifications = ini.GetBool("General", "ShowNotifications", c.ShowNotifications);
                c.AutoStartOnLoad = ini.GetBool("General", "AutoStartOnLoad", c.AutoStartOnLoad);
                c.ThreatRadius = ini.GetFloat("General", "ThreatRadius", c.ThreatRadius);
                c.EngagePolice = ini.GetBool("General", "EngagePolice", c.EngagePolice);
                c.RemoveDeadBodies = ini.GetBool("General", "RemoveDeadBodies", c.RemoveDeadBodies);
                c.DeadBodyCleanupDelay = ini.GetInt("General", "DeadBodyCleanupDelay", c.DeadBodyCleanupDelay);
                c.PlayerDeath = ini.GetEnum("General", "PlayerDeathBehavior", c.PlayerDeath);
                c.TeleportDistance = ini.GetFloat("General", "TeleportDistance", c.TeleportDistance);
                c.RelationshipGroupName = ini.GetString("General", "RelationshipGroupName", c.RelationshipGroupName);
                c.CodeRedEnabled = ini.GetBool("General", "CodeRedEnabled", c.CodeRedEnabled);
                c.CodeRedHealthDropThreshold = ini.GetInt("General", "CodeRedHealthDropThreshold", c.CodeRedHealthDropThreshold);
                c.CodeRedDuration = ini.GetInt("General", "CodeRedDuration", c.CodeRedDuration);
                c.StuckSpeedThreshold = ini.GetFloat("General", "StuckSpeedThreshold", c.StuckSpeedThreshold);
                c.StuckTimeThreshold = ini.GetInt("General", "StuckTimeThreshold", c.StuckTimeThreshold);

                // -------- CloseProtection --------
                c.GuardCount = ini.GetInt("CloseProtection", "GuardCount", c.GuardCount);
                c.MaxGuards = ini.GetInt("CloseProtection", "MaxGuards", c.MaxGuards);
                c.GuardModels = ini.GetList("CloseProtection", "GuardModels", c.GuardModels);
                c.GuardWeapons = ini.GetList("CloseProtection", "Weapons", c.GuardWeapons);
                c.SidearmWeapon = ini.GetString("CloseProtection", "SidearmWeapon", c.SidearmWeapon);
                c.GuardHealth = ini.GetInt("CloseProtection", "Health", c.GuardHealth);
                c.GuardArmor = ini.GetInt("CloseProtection", "Armor", c.GuardArmor);
                c.GuardAccuracy = ini.GetInt("CloseProtection", "Accuracy", c.GuardAccuracy);
                c.GuardShootRate = ini.GetInt("CloseProtection", "ShootRate", c.GuardShootRate);
                c.GuardCombatAbility = ini.GetInt("CloseProtection", "CombatAbility", c.GuardCombatAbility);
                c.GuardCombatRange = ini.GetInt("CloseProtection", "CombatRange", c.GuardCombatRange);
                c.GuardCombatMovement = ini.GetInt("CloseProtection", "CombatMovement", c.GuardCombatMovement);
                c.GuardSeeingRange = ini.GetFloat("CloseProtection", "SeeingRange", c.GuardSeeingRange);
                c.GuardHearingRange = ini.GetFloat("CloseProtection", "HearingRange", c.GuardHearingRange);
                c.GuardInvincible = ini.GetBool("CloseProtection", "Invincible", c.GuardInvincible);
                c.GuardCriticalHits = ini.GetBool("CloseProtection", "SuffersCriticalHits", c.GuardCriticalHits);
                c.GuardInfiniteAmmo = ini.GetBool("CloseProtection", "InfiniteAmmo", c.GuardInfiniteAmmo);
                c.GuardAutoRespawn = ini.GetBool("CloseProtection", "AutoRespawn", c.GuardAutoRespawn);
                c.GuardRespawnDelay = ini.GetInt("CloseProtection", "RespawnDelay", c.GuardRespawnDelay);
                c.GuardFollowSpeed = ini.GetFloat("CloseProtection", "FollowSpeed", c.GuardFollowSpeed);
                c.GuardStoppingRange = ini.GetFloat("CloseProtection", "StoppingRange", c.GuardStoppingRange);
                c.DefaultFormation = ini.GetEnum("CloseProtection", "DefaultFormation", c.DefaultFormation);
                c.DefaultBehavior = ini.GetEnum("CloseProtection", "DefaultBehavior", c.DefaultBehavior);
                c.RandomOutfits = ini.GetBool("CloseProtection", "RandomOutfits", c.RandomOutfits);
                c.OutfitSets = ini.GetList("CloseProtection", "OutfitSets", c.OutfitSets);
                c.GuardBlipColor = ini.GetInt("CloseProtection", "BlipColor", c.GuardBlipColor);
                c.GuardBlipSprite = ini.GetInt("CloseProtection", "BlipSprite", c.GuardBlipSprite);

                // -------- Convoy --------
                c.SpawnVipVehicle = ini.GetBool("Convoy", "SpawnVipVehicle", c.SpawnVipVehicle);
                c.WarpPlayerIntoVip = ini.GetBool("Convoy", "WarpPlayerIntoVip", c.WarpPlayerIntoVip);
                c.VipVehicleModel = ini.GetString("Convoy", "VipVehicleModel", c.VipVehicleModel);
                c.LeadVehicleModels = ini.GetList("Convoy", "LeadVehicleModels", c.LeadVehicleModels);
                c.SupportVehicleModels = ini.GetList("Convoy", "SupportVehicleModels", c.SupportVehicleModels);
                c.LeadVehicleCount = ini.GetInt("Convoy", "LeadVehicleCount", c.LeadVehicleCount);
                c.SupportVehicleCount = ini.GetInt("Convoy", "SupportVehicleCount", c.SupportVehicleCount);
                c.GuardsPerVehicle = ini.GetInt("Convoy", "GuardsPerVehicle", c.GuardsPerVehicle);
                c.SpeedNormal = ini.GetFloat("Convoy", "SpeedNormal", c.SpeedNormal);
                c.SpeedAggressive = ini.GetFloat("Convoy", "SpeedAggressive", c.SpeedAggressive);
                c.SpeedStealth = ini.GetFloat("Convoy", "SpeedStealth", c.SpeedStealth);
                c.DrivingStyleNormal = ini.GetInt("Convoy", "DrivingStyleNormal", c.DrivingStyleNormal);
                c.DrivingStyleAggressive = ini.GetInt("Convoy", "DrivingStyleAggressive", c.DrivingStyleAggressive);
                c.DrivingStyleStealth = ini.GetInt("Convoy", "DrivingStyleStealth", c.DrivingStyleStealth);
                c.ConvoyMinDistance = ini.GetFloat("Convoy", "MinDistance", c.ConvoyMinDistance);
                c.ConvoyNoRoadsDistance = ini.GetFloat("Convoy", "NoRoadsDistance", c.ConvoyNoRoadsDistance);
                c.DeployOnStop = ini.GetBool("Convoy", "DeployOnStop", c.DeployOnStop);
                c.DeployRadius = ini.GetFloat("Convoy", "DeployRadius", c.DeployRadius);
                c.ArmoredVehicles = ini.GetBool("Convoy", "ArmoredVehicles", c.ArmoredVehicles);
                c.VehicleTint = ini.GetInt("Convoy", "WindowTint", c.VehicleTint);
                c.VehiclePrimaryColor = ini.GetInt("Convoy", "PrimaryColor", c.VehiclePrimaryColor);
                c.VehicleSecondaryColor = ini.GetInt("Convoy", "SecondaryColor", c.VehicleSecondaryColor);
                c.ConvoyBlipColor = ini.GetInt("Convoy", "BlipColor", c.ConvoyBlipColor);
                c.ConvoyBlipSprite = ini.GetInt("Convoy", "BlipSprite", c.ConvoyBlipSprite);

                // -------- AirSupport --------
                c.AirVehicleModels = ini.GetList("AirSupport", "AirVehicleModels", c.AirVehicleModels);
                c.DefaultAirVehicle = ini.GetString("AirSupport", "DefaultAirVehicle", c.DefaultAirVehicle);
                c.PilotModel = ini.GetString("AirSupport", "PilotModel", c.PilotModel);
                c.GunnerCount = ini.GetInt("AirSupport", "GunnerCount", c.GunnerCount);
                c.MaxAirUnits = ini.GetInt("AirSupport", "MaxAirUnits", c.MaxAirUnits);
                c.AirHeight = ini.GetFloat("AirSupport", "Height", c.AirHeight);
                c.AirRadius = ini.GetFloat("AirSupport", "Radius", c.AirRadius);
                c.AirSpeed = ini.GetFloat("AirSupport", "Speed", c.AirSpeed);
                c.AirAutoEngage = ini.GetBool("AirSupport", "AutoEngage", c.AirAutoEngage);
                c.AirInvincible = ini.GetBool("AirSupport", "Invincible", c.AirInvincible);
                c.CargobobModel = ini.GetString("AirSupport", "CargobobModel", c.CargobobModel);
                c.LandingTimeout = ini.GetInt("AirSupport", "LandingTimeout", c.LandingTimeout);
                c.AirBlipColor = ini.GetInt("AirSupport", "BlipColor", c.AirBlipColor);
                c.AirBlipSprite = ini.GetInt("AirSupport", "BlipSprite", c.AirBlipSprite);

                // -------- Backup --------
                c.BackupVehicleModels = ini.GetList("Backup", "VehicleModels", c.BackupVehicleModels);
                c.BackupSquadSize = ini.GetInt("Backup", "SquadSize", c.BackupSquadSize);
                c.BackupSpawnDistance = ini.GetFloat("Backup", "SpawnDistance", c.BackupSpawnDistance);
                c.BackupCooldown = ini.GetInt("Backup", "Cooldown", c.BackupCooldown);
            }
            catch (Exception ex)
            {
                Logger.Error("Config.Load hatasi", ex);
            }

            c.Clamp();
            return c;
        }

        /// <summary>Kullanicinin sacma deger girmesine karsi guvenlik siniri.</summary>
        public void Clamp()
        {
            MaxGuards = Math.Max(1, Math.Min(24, MaxGuards));
            GuardCount = Math.Max(1, Math.Min(MaxGuards, GuardCount));
            GuardHealth = Math.Max(100, Math.Min(5000, GuardHealth));
            GuardArmor = Math.Max(0, Math.Min(100, GuardArmor));
            GuardAccuracy = Math.Max(1, Math.Min(100, GuardAccuracy));
            GuardShootRate = Math.Max(10, Math.Min(1000, GuardShootRate));
            GuardCombatAbility = Math.Max(0, Math.Min(2, GuardCombatAbility));
            GuardCombatRange = Math.Max(0, Math.Min(3, GuardCombatRange));
            GuardCombatMovement = Math.Max(0, Math.Min(3, GuardCombatMovement));
            GuardFollowSpeed = Math.Max(1f, Math.Min(3f, GuardFollowSpeed));
            ThreatRadius = Math.Max(15f, Math.Min(200f, ThreatRadius));
            LeadVehicleCount = Math.Max(0, Math.Min(3, LeadVehicleCount));
            SupportVehicleCount = Math.Max(0, Math.Min(4, SupportVehicleCount));
            GuardsPerVehicle = Math.Max(0, Math.Min(3, GuardsPerVehicle));
            GunnerCount = Math.Max(0, Math.Min(4, GunnerCount));
            MaxAirUnits = Math.Max(1, Math.Min(6, MaxAirUnits));
            AirHeight = Math.Max(15f, Math.Min(250f, AirHeight));
            AirRadius = Math.Max(20f, Math.Min(250f, AirRadius));
            LandingTimeout = Math.Max(10000, Math.Min(120000, LandingTimeout));
            BackupSquadSize = Math.Max(1, Math.Min(8, BackupSquadSize));
            BackupSpawnDistance = Math.Max(60f, Math.Min(400f, BackupSpawnDistance));
            CodeRedHealthDropThreshold = Math.Max(5, Math.Min(100, CodeRedHealthDropThreshold));
            CodeRedDuration = Math.Max(2000, Math.Min(60000, CodeRedDuration));
            StuckSpeedThreshold = Math.Max(0.3f, Math.Min(5f, StuckSpeedThreshold));
            StuckTimeThreshold = Math.Max(2000, Math.Min(30000, StuckTimeThreshold));
            ConvoyNoRoadsDistance = Math.Max(5f, Math.Min(60f, ConvoyNoRoadsDistance));

            if (GuardModels == null || GuardModels.Length == 0)
                GuardModels = new[] { "s_m_m_highsec_01" };
            if (GuardWeapons == null || GuardWeapons.Length == 0)
                GuardWeapons = new[] { "CarbineRifle" };
            if (AirVehicleModels == null || AirVehicleModels.Length == 0)
                AirVehicleModels = new[] { "buzzard" };
        }

        // ==================================================================
        // Varsayilan .ini olusturma
        // ==================================================================
        private void WriteDefaultIni(string path)
        {
            try
            {
                string dir = Path.GetDirectoryName(path);
                if (!string.IsNullOrEmpty(dir) && !Directory.Exists(dir))
                    Directory.CreateDirectory(dir);

                File.WriteAllText(path, DefaultIniText(), Encoding.UTF8);
                Logger.Info("Varsayilan ini olusturuldu: " + path);
            }
            catch (Exception ex)
            {
                Logger.Error("Varsayilan ini yazilamadi", ex);
            }
        }

        public static string DefaultIniText()
        {
            StringBuilder sb = new StringBuilder();
            sb.AppendLine("; ==========================================================");
            sb.AppendLine("; Mafia VIP Protection - Ayar Dosyasi");
            sb.AppendLine("; Tus isimleri .NET 'Keys' enum'una gore yazilir (F7, NumPad5, D1, LControlKey ...)");
            sb.AppendLine("; Listeler virgulle ayrilir. Ondalik ayirici nokta veya virgul olabilir.");
            sb.AppendLine("; ==========================================================");
            sb.AppendLine();
            sb.AppendLine("[Keys]");
            sb.AppendLine("MenuKey = F7                 ; Ana menu");
            sb.AppendLine("ModifierKey = None           ; Asagidaki hizli tuslar icin zorunlu modifier (None = yok)");
            sb.AppendLine("GuardsToggleKey = F8         ; Yakin koruma ac/kapa");
            sb.AppendLine("ConvoyToggleKey = F9         ; Konvoy ac/kapa");
            sb.AppendLine("AirSupportToggleKey = F10    ; Hava destegi ac/kapa");
            sb.AppendLine("BackupKey = F11              ; Yedek birlik cagir");
            sb.AppendLine("FormationCycleKey = None     ; Formasyon degistir");
            sb.AppendLine("ClearAllKey = None           ; Tum ekipleri temizle");
            sb.AppendLine();
            sb.AppendLine("[General]");
            sb.AppendLine("EnableBlips = true");
            sb.AppendLine("ShowNotifications = true");
            sb.AppendLine("AutoStartOnLoad = false      ; Oyun acilir acilmaz korumalari spawn et");
            sb.AppendLine("ThreatRadius = 65            ; Tehdit tarama yaricapi (metre)");
            sb.AppendLine("EngagePolice = false         ; Aranma varken polisi de tehdit say");
            sb.AppendLine("RemoveDeadBodies = true");
            sb.AppendLine("DeadBodyCleanupDelay = 25000 ; ms");
            sb.AppendLine("PlayerDeathBehavior = Guard  ; Guard | Scatter | Vanish");
            sb.AppendLine("TeleportDistance = 130       ; Bu mesafeden geride kalan koruma isinlanir");
            sb.AppendLine("RelationshipGroupName = MAFIA_VIP_GUARDS");
            sb.AppendLine("CodeRedEnabled = true        ; Ani can kaybinda tum ekip gecici olarak agresiflesir");
            sb.AppendLine("CodeRedHealthDropThreshold = 35  ; Bu kadar ani can dususu Kod Kirmizi tetikler");
            sb.AppendLine("CodeRedDuration = 12000       ; ms, agresif mod ne kadar surer");
            sb.AppendLine("StuckSpeedThreshold = 1.5     ; Bu hizin altinda arac/koruma 'sikisti' sayilmaya baslar");
            sb.AppendLine("StuckTimeThreshold = 7000     ; ms, bu sure sikisik kalirsa isinlanarak kurtarilir");
            sb.AppendLine();
            sb.AppendLine("[CloseProtection]");
            sb.AppendLine("GuardCount = 6");
            sb.AppendLine("MaxGuards = 8");
            sb.AppendLine("GuardModels = s_m_m_highsec_01, s_m_m_highsec_02, s_m_y_blackops_01, s_m_m_armoured_01, s_m_m_armoured_02");
            sb.AppendLine("Weapons = CarbineRifle, SpecialCarbine, CombatMG, PumpShotgun");
            sb.AppendLine("SidearmWeapon = Pistol");
            sb.AppendLine("Health = 400");
            sb.AppendLine("Armor = 100");
            sb.AppendLine("Accuracy = 75                ; 1-100");
            sb.AppendLine("ShootRate = 800              ; 10-1000");
            sb.AppendLine("CombatAbility = 2            ; 0 zayif, 1 orta, 2 profesyonel");
            sb.AppendLine("CombatRange = 2              ; 0 yakin, 1 orta, 2 uzak");
            sb.AppendLine("CombatMovement = 2           ; 0 sabit, 1 defansif, 2 ilerleyen");
            sb.AppendLine("SeeingRange = 90");
            sb.AppendLine("HearingRange = 110");
            sb.AppendLine("Invincible = false");
            sb.AppendLine("SuffersCriticalHits = false  ; false = kafa vurusu aninda oldurmez");
            sb.AppendLine("InfiniteAmmo = true");
            sb.AppendLine("AutoRespawn = true           ; Olen koruma yerine yenisi gelsin");
            sb.AppendLine("RespawnDelay = 15000         ; ms");
            sb.AppendLine("FollowSpeed = 2              ; 1 yuru, 2 kos, 3 sprint");
            sb.AppendLine("StoppingRange = 1.2");
            sb.AppendLine("DefaultFormation = Diamond   ; Diamond | Box | Column | Wedge | Circle");
            sb.AppendLine("DefaultBehavior = Defensive  ; Aggressive | Defensive | Passive");
            sb.AppendLine("RandomOutfits = true");
            sb.AppendLine("; OutfitSets: her set 'component:drawable:texture' parcalarindan olusur, parcalar | ile ayrilir.");
            sb.AppendLine("; Ornek: OutfitSets = 3:1:0|4:1:0|8:1:0|11:1:0, 3:2:0|4:2:0|8:2:0|11:2:0");
            sb.AppendLine("OutfitSets =");
            sb.AppendLine("BlipColor = 2                ; 2 = yesil");
            sb.AppendLine("BlipSprite = 1");
            sb.AppendLine();
            sb.AppendLine("[Convoy]");
            sb.AppendLine("SpawnVipVehicle = true       ; false: oyuncunun bindigi arac VIP araci sayilir");
            sb.AppendLine("WarpPlayerIntoVip = false    ; true: oyuncu VIP aracina otomatik binsin");
            sb.AppendLine("VipVehicleModel = cognoscenti2");
            sb.AppendLine("LeadVehicleModels = kuruma2, schafter6");
            sb.AppendLine("SupportVehicleModels = baller6, insurgent3, kuruma2");
            sb.AppendLine("LeadVehicleCount = 1         ; 0-3");
            sb.AppendLine("SupportVehicleCount = 2      ; 0-4");
            sb.AppendLine("GuardsPerVehicle = 3         ; Surucu haric 0-3");
            sb.AppendLine("SpeedNormal = 22");
            sb.AppendLine("SpeedAggressive = 38");
            sb.AppendLine("SpeedStealth = 14");
            sb.AppendLine("DrivingStyleNormal = 786603");
            sb.AppendLine("DrivingStyleAggressive = 786469");
            sb.AppendLine("DrivingStyleStealth = 786603");
            sb.AppendLine("MinDistance = 12");
            sb.AppendLine("NoRoadsDistance = 20         ; Bu mesafenin altinda arac yol agini yok sayip dogrudan takip eder");
            sb.AppendLine("DeployOnStop = true          ; Konvoy durunca korumalar insin");
            sb.AppendLine("DeployRadius = 6.5");
            sb.AppendLine("ArmoredVehicles = true");
            sb.AppendLine("WindowTint = 5");
            sb.AppendLine("PrimaryColor = 0");
            sb.AppendLine("SecondaryColor = 0");
            sb.AppendLine("BlipColor = 3                ; 3 = mavi");
            sb.AppendLine("BlipSprite = 225");
            sb.AppendLine();
            sb.AppendLine("[AirSupport]");
            sb.AppendLine("AirVehicleModels = buzzard, savage, valkyrie, annihilator");
            sb.AppendLine("DefaultAirVehicle = buzzard");
            sb.AppendLine("PilotModel = s_m_y_blackops_01");
            sb.AppendLine("GunnerCount = 2              ; NOT: mermili helikopterlerde (buzzard vb.) yolcular silahi ATESLEYEMEZ,");
            sb.AppendLine(";  sadece pilot otomatik ates eder; yolcular yalnizca dusurulurse yerde savasan ek mürettebattir.");
            sb.AppendLine("MaxAirUnits = 3              ; Ayni anda cagrilabilecek hava birimi sayisi");
            sb.AppendLine("Height = 45");
            sb.AppendLine("Radius = 60");
            sb.AppendLine("Speed = 40");
            sb.AppendLine("AutoEngage = true");
            sb.AppendLine("Invincible = false");
            sb.AppendLine("CargobobModel = cargobob3");
            sb.AppendLine("LandingTimeout = 45000       ; ms, bu sureden sonra manuel inis (yere kilitleme) devreye girer");
            sb.AppendLine("BlipColor = 5                ; 5 = sari");
            sb.AppendLine("BlipSprite = 43");
            sb.AppendLine();
            sb.AppendLine("[Backup]");
            sb.AppendLine("VehicleModels = insurgent3, baller6, kuruma2");
            sb.AppendLine("SquadSize = 4");
            sb.AppendLine("SpawnDistance = 160");
            sb.AppendLine("Cooldown = 45000");
            return sb.ToString();
        }
    }
}
