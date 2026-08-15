using System;
using System.Collections.Generic;
using GTA;
using LemonUI;
using LemonUI.Menus;

namespace MafiaVIP
{
    /// <summary>
    /// LemonUI tabanli menu sistemi.
    ///
    /// Yakin Koruma / Konvoy / Hava Destegi / Genel olmak uzere dort ana bolum.
    /// Menu her karede Process() ile islenir; agir islem yapmaz.
    /// </summary>
    internal sealed class MenuSystem
    {
        private readonly Config _cfg;
        private readonly CloseProtection _guards;
        private readonly ConvoyManager _convoy;
        private readonly AirSupport _air;
        private readonly BackupManager _backup;
        private readonly Action _clearAll;
        private readonly Action _reloadConfig;

        private readonly ObjectPool _pool = new ObjectPool();
        private NativeMenu _main;
        private NativeItem _statusItem;
        private int _lastStatusUpdate;

        public MenuSystem(Config cfg, CloseProtection guards, ConvoyManager convoy, AirSupport air,
                          BackupManager backup, Action clearAll, Action reloadConfig)
        {
            _cfg = cfg;
            _guards = guards;
            _convoy = convoy;
            _air = air;
            _backup = backup;
            _clearAll = clearAll;
            _reloadConfig = reloadConfig;

            Build();
        }

        public bool Visible
        {
            get { return _main != null && _main.Visible; }
            set { if (_main != null) _main.Visible = value; }
        }

        public void Toggle()
        {
            if (_main == null) return;
            _main.Visible = !_main.Visible;
        }

        public void Process()
        {
            _pool.Process();

            // Durum satirini yalnizca menu acikken ve saniyede bir guncelle.
            if (_main != null && _main.Visible && Game.GameTime - _lastStatusUpdate > 1000)
            {
                _lastStatusUpdate = Game.GameTime;
                RefreshStatus();
            }
        }

        private void RefreshStatus()
        {
            if (_statusItem == null) return;

            _statusItem.Title = string.Format("Durum: {0} koruma | Konvoy {1} | Hava {2}",
                _guards.AliveCount,
                _convoy.Active ? _convoy.UnitCount + " arac" : "kapali",
                _air.Active ? "aktif" : "kapali");
        }

        // ==================================================================
        // Menu insasi
        // ==================================================================
        private void Build()
        {
            _main = new NativeMenu("MAFIA VIP", "KORUMA SISTEMI");
            _pool.Add(_main);

            NativeMenu guardsMenu = BuildGuardsMenu();
            NativeMenu convoyMenu = BuildConvoyMenu();
            NativeMenu airMenu = BuildAirMenu();
            NativeMenu generalMenu = BuildGeneralMenu();

            _main.AddSubMenu(guardsMenu);
            _main.AddSubMenu(convoyMenu);
            _main.AddSubMenu(airMenu);
            _main.AddSubMenu(generalMenu);

            _statusItem = new NativeItem("Durum: -", "Aktif birimlerin ozeti.");
            _statusItem.Enabled = false;
            _main.Add(_statusItem);
        }

        // ------------------------------------------------------------------
        // 1) Yakin koruma
        // ------------------------------------------------------------------
        private NativeMenu BuildGuardsMenu()
        {
            NativeMenu menu = new NativeMenu("Yakin Koruma", "YAKIN KORUMA EKIBI");
            _pool.Add(menu);

            NativeItem deploy = new NativeItem("Ekibi Cagir", "Ayarlanan sayida korumayi goreve alir.");
            deploy.Activated += (s, e) => Safe(() => _guards.Deploy());
            menu.Add(deploy);

            NativeItem dismiss = new NativeItem("Ekibi Dagit", "Tum korumalari gorevden alir ve siler.");
            dismiss.Activated += (s, e) => Safe(() => _guards.Dismiss());
            menu.Add(dismiss);

            // Sayi
            int[] counts = new int[_cfg.MaxGuards];
            for (int i = 0; i < counts.Length; i++) counts[i] = i + 1;

            NativeListItem<int> countItem = new NativeListItem<int>("Koruma Sayisi",
                "Ekipteki koruma sayisi.", counts);
            countItem.SelectedIndex = Math.Max(0, Math.Min(counts.Length - 1, _guards.TargetCount - 1));
            countItem.ItemChanged += (s, e) => Safe(() => _guards.SetCount(countItem.SelectedItem));
            menu.Add(countItem);

            // Silah
            List<string> weapons = new List<string> { "Rastgele" };
            weapons.AddRange(_cfg.GuardWeapons);

            NativeListItem<string> weaponItem = new NativeListItem<string>("Silah",
                "Ekibin ana silahi.", weapons.ToArray());
            weaponItem.ItemChanged += (s, e) => Safe(() =>
            {
                string choice = weaponItem.SelectedItem;
                _guards.SetWeapon(choice == "Rastgele" ? null : choice);
            });
            menu.Add(weaponItem);

            // Davranis
            NativeListItem<string> behaviorItem = new NativeListItem<string>("Davranis",
                "Agresif: menzildeki tehditlere saldirir. Defansif: sadece karsilik verir. Pasif: ates etmez.",
                "Agresif", "Defansif", "Pasif");
            behaviorItem.SelectedIndex = _guards.Behavior == GuardBehavior.Aggressive ? 0
                                       : _guards.Behavior == GuardBehavior.Defensive ? 1 : 2;
            behaviorItem.ItemChanged += (s, e) => Safe(() =>
            {
                GuardBehavior behavior = behaviorItem.SelectedIndex == 0 ? GuardBehavior.Aggressive
                                       : behaviorItem.SelectedIndex == 1 ? GuardBehavior.Defensive
                                       : GuardBehavior.Passive;
                _guards.SetBehavior(behavior);
                _convoy.Behavior = behavior;
            });
            menu.Add(behaviorItem);

            // Formasyon
            NativeListItem<string> formationItem = new NativeListItem<string>("Formasyon",
                "Korumalarin oyuncu etrafindaki dizilimi.",
                "Elmas", "Kutu", "Kolon", "Kama", "Cember (360)");
            formationItem.SelectedIndex = (int)_guards.Formation;
            formationItem.ItemChanged += (s, e) => Safe(() =>
                _guards.SetFormation((Formation)formationItem.SelectedIndex));
            menu.Add(formationItem);

            // Yedek koruma
            NativeCheckboxItem respawnItem = new NativeCheckboxItem("Otomatik Yedek",
                "Olen korumanin yerine yenisi gelir.", _cfg.GuardAutoRespawn);
            respawnItem.CheckboxChanged += (s, e) => Safe(() =>
            {
                _cfg.GuardAutoRespawn = respawnItem.Checked;
                Utils.Notify("Otomatik yedek: " + (respawnItem.Checked ? "acik" : "kapali"));
            });
            menu.Add(respawnItem);

            return menu;
        }

        // ------------------------------------------------------------------
        // 2) Konvoy
        // ------------------------------------------------------------------
        private NativeMenu BuildConvoyMenu()
        {
            NativeMenu menu = new NativeMenu("Konvoy", "KONVOY / MOTORCADE");
            _pool.Add(menu);

            NativeItem create = new NativeItem("Konvoy Olustur",
                "VIP araci + lead ve destek araclarini goreve alir.");
            create.Activated += (s, e) => Safe(() => _convoy.Create());
            menu.Add(create);

            NativeItem dismiss = new NativeItem("Konvoyu Dagit", "Tum konvoy araclarini siler.");
            dismiss.Activated += (s, e) => Safe(() => _convoy.Dismiss());
            menu.Add(dismiss);

            NativeListItem<string> speedItem = new NativeListItem<string>("Hiz Modu",
                "Normal: trafige uyar. Agresif: hizli ve kurallari yok sayar. Sessiz: yavas ve dikkat cekmez.",
                "Normal", "Agresif", "Sessiz");
            speedItem.SelectedIndex = (int)_convoy.SpeedMode;
            speedItem.ItemChanged += (s, e) => Safe(() =>
                _convoy.SetSpeedMode((ConvoySpeedMode)speedItem.SelectedIndex));
            menu.Add(speedItem);

            NativeListItem<string> formationItem = new NativeListItem<string>("Dizilim",
                "Konvoy araclarinin VIP aracina gore pozisyonu.",
                "Elmas", "Kutu", "Kolon", "Kama", "Cember");
            formationItem.SelectedIndex = (int)_convoy.ConvoyFormation;
            formationItem.ItemChanged += (s, e) => Safe(() =>
                _convoy.SetFormation((Formation)formationItem.SelectedIndex));
            menu.Add(formationItem);

            // VIP araci secimi: ini'deki tum arac modelleri
            List<string> vipModels = new List<string>();
            vipModels.Add(_cfg.VipVehicleModel);
            AddUnique(vipModels, _cfg.LeadVehicleModels);
            AddUnique(vipModels, _cfg.SupportVehicleModels);

            NativeListItem<string> vipItem = new NativeListItem<string>("VIP Araci",
                "Secince yeni VIP araci spawn edilir.", vipModels.ToArray());
            vipItem.Activated += (s, e) => Safe(() => _convoy.ChangeVipVehicle(vipItem.SelectedItem));
            menu.Add(vipItem);

            NativeItem deploy = new NativeItem("Ekibi Indir (360 Savunma)",
                "Konvoy ekibi inip cevre guvenligi alir.");
            deploy.Activated += (s, e) => Safe(() =>
            {
                Ped player = Game.Player.Character;
                Entity center = player.IsInVehicle() ? (Entity)player.CurrentVehicle : player;
                _convoy.DeployCrew(center);
            });
            menu.Add(deploy);

            NativeItem regroup = new NativeItem("Ekibi Topla", "Konvoy ekibi araclara geri biner.");
            regroup.Activated += (s, e) => Safe(() => _convoy.RegroupCrew());
            menu.Add(regroup);

            return menu;
        }

        // ------------------------------------------------------------------
        // 3) Hava destegi
        // ------------------------------------------------------------------
        private NativeMenu BuildAirMenu()
        {
            NativeMenu menu = new NativeMenu("Hava Destegi", "HAVA SAVUNMA VE DESTEK");
            _pool.Add(menu);

            NativeItem call = new NativeItem("Hava Destegi Cagir", "Secili helikopteri goreve alir.");
            call.Activated += (s, e) => Safe(() => _air.Call());
            menu.Add(call);

            NativeItem dismiss = new NativeItem("Geri Gonder", "Helikopter ussune doner.");
            dismiss.Activated += (s, e) => Safe(() => _air.Dismiss());
            menu.Add(dismiss);

            NativeListItem<string> typeItem = new NativeListItem<string>("Hava Araci",
                "Kullanilacak helikopter modeli.", _cfg.AirVehicleModels);
            int typeIndex = Array.IndexOf(_cfg.AirVehicleModels, _air.CurrentModel);
            typeItem.SelectedIndex = typeIndex >= 0 ? typeIndex : 0;
            typeItem.ItemChanged += (s, e) => Safe(() => _air.SetModel(typeItem.SelectedItem));
            menu.Add(typeItem);

            // Yukseklik: 20-200 m
            int[] heights = BuildRange(20, 200, 10);
            NativeListItem<int> heightItem = new NativeListItem<int>("Yukseklik (m)",
                "Devriye ucus yuksekligi.", heights);
            heightItem.SelectedIndex = NearestIndex(heights, (int)_air.Height);
            heightItem.ItemChanged += (s, e) => Safe(() => _air.SetHeight(heightItem.SelectedItem));
            menu.Add(heightItem);

            // Yaricap: 30-200 m
            int[] radii = BuildRange(30, 200, 10);
            NativeListItem<int> radiusItem = new NativeListItem<int>("Mesafe (m)",
                "Oyuncunun etrafinda cizilen daire yaricapi.", radii);
            radiusItem.SelectedIndex = NearestIndex(radii, (int)_air.Radius);
            radiusItem.ItemChanged += (s, e) => Safe(() => _air.SetRadius(radiusItem.SelectedItem));
            menu.Add(radiusItem);

            NativeCheckboxItem engageItem = new NativeCheckboxItem("Otomatik Engaje",
                "Tehditleri kendiliginden hedef alir.", _air.AutoEngage);
            engageItem.CheckboxChanged += (s, e) => Safe(() =>
            {
                _air.AutoEngage = engageItem.Checked;
                Utils.Notify("Hava destegi otomatik engaje: " + (engageItem.Checked ? "acik" : "kapali"));
            });
            menu.Add(engageItem);

            NativeItem extraction = new NativeItem("Cargobob - Tahliye",
                "Cargobob gelir, yaniniza iner ve sizi bekler.");
            extraction.Activated += (s, e) => Safe(() => _air.RequestExtraction());
            menu.Add(extraction);

            NativeItem troopDrop = new NativeItem("Cargobob - Birlik Indir",
                "Havadan takviye koruma birligi indirir.");
            troopDrop.Activated += (s, e) => Safe(() => _air.RequestTroopDrop(_guards.Adopt));
            menu.Add(troopDrop);

            return menu;
        }

        // ------------------------------------------------------------------
        // 4) Genel
        // ------------------------------------------------------------------
        private NativeMenu BuildGeneralMenu()
        {
            NativeMenu menu = new NativeMenu("Genel", "GENEL AYARLAR");
            _pool.Add(menu);

            NativeItem backup = new NativeItem("Yedek Birlik Cagir (Kara)",
                "Uzaktan zirhli arac ile takviye ekip gelir.");
            backup.Activated += (s, e) => Safe(() => _backup.Call(_guards.Adopt));
            menu.Add(backup);

            NativeItem clear = new NativeItem("Tum Ekipleri Temizle",
                "Koruma, konvoy ve hava destegini siler.");
            clear.Activated += (s, e) => Safe(() => { if (_clearAll != null) _clearAll(); });
            menu.Add(clear);

            NativeCheckboxItem blipItem = new NativeCheckboxItem("Blipler",
                "Harita isaretlerini acar/kapatir (yeni birimler icin gecerli).", _cfg.EnableBlips);
            blipItem.CheckboxChanged += (s, e) => Safe(() => _cfg.EnableBlips = blipItem.Checked);
            menu.Add(blipItem);

            NativeCheckboxItem notifyItem = new NativeCheckboxItem("Bildirimler",
                "Ekran bildirimlerini acar/kapatir.", _cfg.ShowNotifications);
            notifyItem.CheckboxChanged += (s, e) => Safe(() =>
            {
                _cfg.ShowNotifications = notifyItem.Checked;
                Utils.NotificationsEnabled = notifyItem.Checked;
            });
            menu.Add(notifyItem);

            NativeCheckboxItem policeItem = new NativeCheckboxItem("Polisi Tehdit Say",
                "Aranma seviyesi varken polisle de catisirlar.", _cfg.EngagePolice);
            policeItem.CheckboxChanged += (s, e) => Safe(() => _cfg.EngagePolice = policeItem.Checked);
            menu.Add(policeItem);

            NativeItem reload = new NativeItem("Ayarlari Yeniden Yukle",
                "MafiaVIPProtection.ini dosyasini tekrar okur (tum birimler temizlenir).");
            reload.Activated += (s, e) => Safe(() => { if (_reloadConfig != null) _reloadConfig(); });
            menu.Add(reload);

            menu.AddSubMenu(BuildHelpMenu());
            return menu;
        }

        private NativeMenu BuildHelpMenu()
        {
            NativeMenu menu = new NativeMenu("Yardim", "TUS ATAMALARI");
            _pool.Add(menu);

            string modifier = _cfg.ModifierKey == System.Windows.Forms.Keys.None
                ? string.Empty
                : _cfg.ModifierKey + " + ";

            AddInfo(menu, "Menu", _cfg.MenuKey.ToString());
            AddInfo(menu, "Yakin koruma ac/kapa", modifier + _cfg.GuardsToggleKey);
            AddInfo(menu, "Konvoy ac/kapa", modifier + _cfg.ConvoyToggleKey);
            AddInfo(menu, "Hava destegi ac/kapa", modifier + _cfg.AirSupportToggleKey);
            AddInfo(menu, "Yedek birlik", modifier + _cfg.BackupKey);
            AddInfo(menu, "Formasyon degistir", modifier + _cfg.FormationCycleKey);
            AddInfo(menu, "Tumunu temizle", modifier + _cfg.ClearAllKey);
            AddInfo(menu, "Ayar dosyasi", "scripts/MafiaVIPProtection.ini");

            return menu;
        }

        private void AddInfo(NativeMenu menu, string title, string value)
        {
            NativeItem item = new NativeItem(title + ":  " + value, value);
            item.Enabled = false;
            menu.Add(item);
        }

        // ------------------------------------------------------------------
        // Yardimcilar
        // ------------------------------------------------------------------
        private static void AddUnique(List<string> list, string[] values)
        {
            if (values == null) return;
            foreach (string value in values)
            {
                if (string.IsNullOrEmpty(value)) continue;
                if (!list.Contains(value)) list.Add(value);
            }
        }

        private static int[] BuildRange(int from, int to, int step)
        {
            List<int> values = new List<int>();
            for (int value = from; value <= to; value += step) values.Add(value);
            return values.ToArray();
        }

        private static int NearestIndex(int[] values, int target)
        {
            int bestIndex = 0;
            int bestDelta = int.MaxValue;

            for (int i = 0; i < values.Length; i++)
            {
                int delta = Math.Abs(values[i] - target);
                if (delta < bestDelta)
                {
                    bestDelta = delta;
                    bestIndex = i;
                }
            }
            return bestIndex;
        }

        /// <summary>Menu geri cagrilarindan gelen hatalar oyunu dusurmesin.</summary>
        private static void Safe(Action action)
        {
            try { action(); }
            catch (Exception ex)
            {
                Logger.Error("Menu islemi hatasi", ex);
                Utils.Notify("Islem sirasinda hata olustu (log'a bakin).");
            }
        }
    }
}
