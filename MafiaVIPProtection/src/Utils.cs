using System;
using System.Collections.Generic;
using GTA;
using GTA.Math;
using GTA.Native;

namespace MafiaVIP
{
    /// <summary>Genel amacli yardimci fonksiyonlar.</summary>
    internal static class Utils
    {
        public static readonly Random Rng = new Random(Environment.TickCount);

        // ------------------------------------------------------------------
        // Bildirim
        // ------------------------------------------------------------------
        public static bool NotificationsEnabled = true;

        public static void Notify(string message)
        {
            if (!NotificationsEnabled) return;
            try
            {
                GTA.UI.Notification.Show("~y~[KORUMA]~s~ " + message);
            }
            catch (Exception ex)
            {
                Logger.Error("Notify hatasi", ex);
            }
        }

        public static void Subtitle(string message, int durationMs = 2500)
        {
            try
            {
                GTA.UI.Screen.ShowSubtitle(message, durationMs);
            }
            catch { /* onemli degil */ }
        }

        // ------------------------------------------------------------------
        // Varlik gecerlilik kontrolleri
        // ------------------------------------------------------------------
        public static bool Valid(Entity entity)
        {
            try { return entity != null && entity.Exists(); }
            catch { return false; }
        }

        public static bool AlivePed(Ped ped)
        {
            try { return ped != null && ped.Exists() && ped.IsAlive; }
            catch { return false; }
        }

        public static bool DrivableVehicle(Vehicle vehicle)
        {
            try { return vehicle != null && vehicle.Exists() && !vehicle.IsDead && vehicle.IsDriveable; }
            catch { return false; }
        }

        // ------------------------------------------------------------------
        // Model yukleme
        // ------------------------------------------------------------------
        /// <summary>
        /// Modeli talep eder ve yuklenmesini bekler. Basarisiz olursa null doner.
        /// Model gecerli degilse (yanlis isim / eksik add-on) sessizce null.
        /// </summary>
        public static Model? RequestModel(string name, int timeoutMs = 4000)
        {
            if (string.IsNullOrEmpty(name)) return null;

            try
            {
                Model model = new Model(name.Trim());
                if (!model.IsValid || !model.IsInCdImage)
                {
                    Logger.Warn("Gecersiz model: " + name);
                    return null;
                }

                model.Request();
                int deadline = Game.GameTime + timeoutMs;
                while (!model.IsLoaded && Game.GameTime < deadline)
                    Script.Yield();

                if (!model.IsLoaded)
                {
                    Logger.Warn("Model yuklenemedi (timeout): " + name);
                    return null;
                }

                return model;
            }
            catch (Exception ex)
            {
                Logger.Error("RequestModel hatasi: " + name, ex);
                return null;
            }
        }

        /// <summary>Listeden rastgele bir model dener; hicbiri yuklenemezse null.</summary>
        public static Model? RequestAnyModel(string[] names)
        {
            if (names == null || names.Length == 0) return null;

            // Rastgele bir baslangictan itibaren tum listeyi dene.
            int start = Rng.Next(names.Length);
            for (int i = 0; i < names.Length; i++)
            {
                Model? model = RequestModel(names[(start + i) % names.Length]);
                if (model.HasValue) return model;
            }
            return null;
        }

        // ------------------------------------------------------------------
        // Silah cozumleme
        // ------------------------------------------------------------------
        /// <summary>
        /// "CarbineRifle" veya "weapon_carbinerifle" gibi girdileri WeaponHash'e cevirir.
        /// </summary>
        public static WeaponHash ResolveWeapon(string name, WeaponHash fallback = WeaponHash.CarbineRifle)
        {
            if (string.IsNullOrEmpty(name)) return fallback;

            string trimmed = name.Trim();
            WeaponHash parsed;
            if (Enum.TryParse(trimmed, true, out parsed))
                return parsed;

            try
            {
                string key = trimmed.ToLowerInvariant();
                if (!key.StartsWith("weapon_")) key = "weapon_" + key;
                int hash = Game.GenerateHash(key);
                if (hash != 0) return (WeaponHash)hash;
            }
            catch (Exception ex)
            {
                Logger.Error("ResolveWeapon hatasi: " + name, ex);
            }

            return fallback;
        }

        // ------------------------------------------------------------------
        // Blip
        // ------------------------------------------------------------------
        public static Blip AttachBlip(Entity entity, int sprite, int color, string name, float scale = 0.75f, bool shortRange = true)
        {
            if (!Valid(entity)) return null;

            try
            {
                Blip blip = entity.AddBlip();
                if (blip == null || !blip.Exists()) return null;

                N.SetBlipSprite(blip.Handle, sprite);
                N.SetBlipColour(blip.Handle, color);
                N.SetBlipScale(blip.Handle, scale);
                N.SetBlipShortRange(blip.Handle, shortRange);
                N.SetBlipName(blip.Handle, name);
                return blip;
            }
            catch (Exception ex)
            {
                Logger.Error("AttachBlip hatasi", ex);
                return null;
            }
        }

        public static void RemoveBlip(Blip blip)
        {
            try { if (blip != null && blip.Exists()) blip.Delete(); }
            catch { /* zaten yok */ }
        }

        // ------------------------------------------------------------------
        // Silme / temizleme
        // ------------------------------------------------------------------
        public static void DeleteEntity(Entity entity)
        {
            try
            {
                if (entity != null && entity.Exists())
                {
                    entity.IsPersistent = false;
                    entity.Delete();
                }
            }
            catch (Exception ex)
            {
                Logger.Error("DeleteEntity hatasi", ex);
            }
        }

        public static void Release(Entity entity)
        {
            try
            {
                if (entity != null && entity.Exists())
                    entity.MarkAsNoLongerNeeded();
            }
            catch { /* onemli degil */ }
        }

        // ------------------------------------------------------------------
        // Konum yardimcilari
        // ------------------------------------------------------------------
        /// <summary>Yayanin guvenle durabilecegi bir zemin noktasi bulur.</summary>
        public static Vector3 SafeGround(Vector3 position)
        {
            Vector3 result;
            if (N.GetSafeCoordForPed(position, true, out result))
                return result;

            float groundZ = N.GetGroundZ(position, position.Z);
            return new Vector3(position.X, position.Y, groundZ);
        }

        /// <summary>En yakin yol dugumunu bulur (arac spawn'i icin).</summary>
        public static Vector3 StreetNode(Vector3 position, int nth = 1)
        {
            Vector3 node;
            if (N.GetNthClosestVehicleNode(position, Math.Max(1, nth), out node))
                return node;
            return position;
        }

        /// <summary>Verilen entity'ye gore lokal offset -> dunya koordinati.</summary>
        public static Vector3 OffsetOf(Entity entity, Vector3 offset)
        {
            if (!Valid(entity)) return Vector3.Zero;
            return N.GetOffsetInWorldCoords(entity.Handle, offset);
        }

        /// <summary>Iki nokta arasindaki yatay mesafe (Z farkini yok sayar).</summary>
        public static float FlatDistance(Vector3 a, Vector3 b)
        {
            float dx = a.X - b.X;
            float dy = a.Y - b.Y;
            return (float)Math.Sqrt(dx * dx + dy * dy);
        }

        /// <summary>Kaynaktan hedefe bakan heading (derece).</summary>
        public static float HeadingTowards(Vector3 from, Vector3 to)
        {
            Vector3 dir = to - from;
            float heading = (float)(Math.Atan2(dir.Y, dir.X) * (180.0 / Math.PI)) - 90f;
            while (heading < 0f) heading += 360f;
            while (heading >= 360f) heading -= 360f;
            return heading;
        }

        /// <summary>Cember uzerinde esit araliklarla dagitilmis nokta.</summary>
        public static Vector3 PointOnCircle(Vector3 center, float radius, int index, int total)
        {
            if (total <= 0) total = 1;
            double angle = (2.0 * Math.PI / total) * index;
            return new Vector3(
                center.X + (float)Math.Cos(angle) * radius,
                center.Y + (float)Math.Sin(angle) * radius,
                center.Z);
        }

        /// <summary>Listeden rastgele eleman.</summary>
        public static T Pick<T>(IList<T> list)
        {
            if (list == null || list.Count == 0) return default(T);
            return list[Rng.Next(list.Count)];
        }
    }
}
