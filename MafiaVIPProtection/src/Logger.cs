using System;
using System.IO;

namespace MafiaVIP
{
    /// <summary>
    /// Basit dosya log'u. Hata ayiklama icin scripts/MafiaVIPProtection.log
    /// dosyasina yazar. Log yazimi asla oyunu dusurmemeli, bu yuzden her sey
    /// try/catch icinde.
    /// </summary>
    internal static class Logger
    {
        private static readonly object Sync = new object();
        private static string _path;
        private static bool _initialized;

        public static bool Enabled = true;

        private static string Path_
        {
            get
            {
                if (_path == null)
                {
                    _path = Path.Combine(AppDomain.CurrentDomain.BaseDirectory, "scripts", "MafiaVIPProtection.log");
                }
                return _path;
            }
        }

        public static void Info(string message)
        {
            Write("INFO ", message);
        }

        public static void Warn(string message)
        {
            Write("WARN ", message);
        }

        public static void Error(string message, Exception ex = null)
        {
            Write("ERROR", ex == null ? message : message + " :: " + ex);
        }

        private static void Write(string level, string message)
        {
            if (!Enabled) return;

            try
            {
                lock (Sync)
                {
                    if (!_initialized)
                    {
                        _initialized = true;
                        // Log dosyasi buyumesin diye her oturumda sifirla.
                        try { if (File.Exists(Path_)) File.Delete(Path_); }
                        catch { /* dosya kilitliyse onemli degil */ }
                    }

                    File.AppendAllText(Path_,
                        string.Format("[{0:HH:mm:ss}] [{1}] {2}{3}",
                            DateTime.Now, level, message, Environment.NewLine));
                }
            }
            catch
            {
                // Loglama hatasi sessizce yutulur.
            }
        }
    }
}
