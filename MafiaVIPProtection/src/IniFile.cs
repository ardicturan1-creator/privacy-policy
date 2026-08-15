using System;
using System.Collections.Generic;
using System.Globalization;
using System.IO;
using System.Windows.Forms;

namespace MafiaVIP
{
    /// <summary>
    /// Bagimsiz, hafif .ini okuyucu.
    /// SHVDN'in ScriptSettings sinifina bagimli degiliz; boylece enum/tus
    /// donusumlerini kendimiz kontrol ediyoruz ve dosya yoksa varsayilanlarla
    /// olusturabiliyoruz.
    /// </summary>
    public sealed class IniFile
    {
        private readonly Dictionary<string, Dictionary<string, string>> _data =
            new Dictionary<string, Dictionary<string, string>>(StringComparer.OrdinalIgnoreCase);

        public bool Loaded { get; private set; }

        public void Load(string path)
        {
            _data.Clear();
            Loaded = false;

            if (string.IsNullOrEmpty(path) || !File.Exists(path))
                return;

            string section = "General";
            foreach (string rawLine in File.ReadAllLines(path))
            {
                string line = rawLine.Trim();
                if (line.Length == 0) continue;
                if (line.StartsWith(";") || line.StartsWith("#") || line.StartsWith("//")) continue;

                if (line.StartsWith("[") && line.EndsWith("]"))
                {
                    section = line.Substring(1, line.Length - 2).Trim();
                    continue;
                }

                int eq = line.IndexOf('=');
                if (eq <= 0) continue;

                string key = line.Substring(0, eq).Trim();
                string value = line.Substring(eq + 1).Trim();

                // Satir sonu yorumlarini temizle ("Value = 5   ; aciklama")
                int comment = value.IndexOf(';');
                if (comment >= 0) value = value.Substring(0, comment).Trim();

                if (!_data.ContainsKey(section))
                    _data[section] = new Dictionary<string, string>(StringComparer.OrdinalIgnoreCase);

                _data[section][key] = value;
            }

            Loaded = true;
        }

        private string Raw(string section, string key)
        {
            Dictionary<string, string> sec;
            if (!_data.TryGetValue(section, out sec)) return null;
            string value;
            return sec.TryGetValue(key, out value) ? value : null;
        }

        public string GetString(string section, string key, string fallback)
        {
            string raw = Raw(section, key);
            return string.IsNullOrEmpty(raw) ? fallback : raw;
        }

        public int GetInt(string section, string key, int fallback)
        {
            string raw = Raw(section, key);
            int result;
            if (!string.IsNullOrEmpty(raw) && int.TryParse(raw, NumberStyles.Integer, CultureInfo.InvariantCulture, out result))
                return result;
            return fallback;
        }

        public float GetFloat(string section, string key, float fallback)
        {
            string raw = Raw(section, key);
            if (string.IsNullOrEmpty(raw)) return fallback;

            float result;
            // Hem "1.5" hem "1,5" yazimini kabul et.
            if (float.TryParse(raw.Replace(',', '.'), NumberStyles.Float, CultureInfo.InvariantCulture, out result))
                return result;
            return fallback;
        }

        public bool GetBool(string section, string key, bool fallback)
        {
            string raw = Raw(section, key);
            if (string.IsNullOrEmpty(raw)) return fallback;

            switch (raw.Trim().ToLowerInvariant())
            {
                case "1":
                case "true":
                case "yes":
                case "on":
                case "evet":
                    return true;
                case "0":
                case "false":
                case "no":
                case "off":
                case "hayir":
                    return false;
                default:
                    return fallback;
            }
        }

        public Keys GetKey(string section, string key, Keys fallback)
        {
            string raw = Raw(section, key);
            if (string.IsNullOrEmpty(raw)) return fallback;

            Keys result;
            if (Enum.TryParse(raw.Trim(), true, out result))
                return result;
            return fallback;
        }

        public T GetEnum<T>(string section, string key, T fallback) where T : struct
        {
            string raw = Raw(section, key);
            if (string.IsNullOrEmpty(raw)) return fallback;

            T result;
            if (Enum.TryParse(raw.Trim(), true, out result))
                return result;
            return fallback;
        }

        /// <summary>Virgulle ayrilmis listeyi diziye cevirir; bos girdileri atar.</summary>
        public string[] GetList(string section, string key, string[] fallback)
        {
            string raw = Raw(section, key);
            if (string.IsNullOrEmpty(raw)) return fallback;

            string[] parts = raw.Split(',');
            List<string> cleaned = new List<string>();
            foreach (string part in parts)
            {
                string trimmed = part.Trim();
                if (trimmed.Length > 0) cleaned.Add(trimmed);
            }
            return cleaned.Count > 0 ? cleaned.ToArray() : fallback;
        }
    }
}
