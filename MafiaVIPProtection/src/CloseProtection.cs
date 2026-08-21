using System;
using System.Collections.Generic;
using GTA;
using GTA.Math;

namespace MafiaVIP
{
    /// <summary>
    /// Yakin koruma ekibi (Close Protection Team).
    ///
    /// - Oyuncunun etrafinda dogal formasyon kurar (sol / sag / arka).
    /// - Oyuncu araca binince bos koltuklara gecer, inince araci terk eder.
    /// - Tehdit algilandiginda otomatik karsilik verir.
    /// - Olen korumanin yerine (ayar aciksa) yenisi gelir.
    /// </summary>
    internal sealed class CloseProtection
    {
        // ------------------------------------------------------------------
        // Tek bir korumanin durumu
        // ------------------------------------------------------------------
        private enum GuardState { Idle, Following, Fighting, Boarding, InVehicle, Deployed, Mourning }

        private sealed class Guard
        {
            public Ped Ped;
            public Blip Blip;
            public GuardState State = GuardState.Idle;
            public int SlotIndex;
            public int LastTaskTime;
            public int TargetHandle;
            public int SeatIndex = -99;
            public int BoardingSince;
            public int DeathTime;          // 0 = henuz olum kaydedilmedi
        }

        private readonly Config _cfg;
        private readonly ThreatScanner _scanner;
        private readonly List<Guard> _guards = new List<Guard>();
        private readonly List<Ped> _pedView = new List<Ped>();
        private readonly List<int> _respawnQueue = new List<int>();
        private readonly HashSet<int> _reservedSeats = new HashSet<int>();

        private bool _playerWasDead;
        private int _lastPedViewBuild;
        private int _lastPlayerHealth = -1;
        private int _codeRedUntil;

        /// <summary>Konvoy aktifse bos koltuklu destek araci saglar (Main tarafindan baglanir).</summary>
        public Func<Vehicle> SupportVehicleProvider;

        public CloseProtection(Config cfg, ThreatScanner scanner)
        {
            _cfg = cfg;
            _scanner = scanner;
            Formation = cfg.DefaultFormation;
            Behavior = cfg.DefaultBehavior;
            TargetCount = cfg.GuardCount;
        }

        // ------------------------------------------------------------------
        // Durum
        // ------------------------------------------------------------------
        public bool Active { get; private set; }
        public Formation Formation { get; private set; }
        public GuardBehavior Behavior { get; private set; }
        public int TargetCount { get; private set; }
        public string WeaponOverride { get; private set; }   // null = ayardan rastgele
        public int AliveCount { get { return LivingCount(); } }

        /// <summary>Kod Kirmizi aktifken (ani can kaybi sonrasi) Pasif/Defansif farketmeksizin Agresif davranilir.</summary>
        public GuardBehavior EffectiveBehavior
        {
            get
            {
                if (_cfg.CodeRedEnabled && Game.GameTime < _codeRedUntil) return GuardBehavior.Aggressive;
                return Behavior;
            }
        }

        /// <summary>Listedeki canli koruma sayisi (temizlenmeyi bekleyen cesetler haric).</summary>
        private int LivingCount()
        {
            int count = 0;
            for (int i = 0; i < _guards.Count; i++)
                if (Utils.AlivePed(_guards[i].Ped)) count++;
            return count;
        }

        /// <summary>Tehdit tarayicisinin kullandigi canli koruma listesi.</summary>
        public IList<Ped> Peds
        {
            get
            {
                // Listeyi her cagrida yeniden kurmamak icin kisa sureli cache.
                if (Game.GameTime - _lastPedViewBuild < 250) return _pedView;

                _lastPedViewBuild = Game.GameTime;
                _pedView.Clear();
                for (int i = 0; i < _guards.Count; i++)
                {
                    if (Utils.AlivePed(_guards[i].Ped))
                        _pedView.Add(_guards[i].Ped);
                }
                return _pedView;
            }
        }

        // ==================================================================
        // Kurulum / dagitma
        // ==================================================================
        public void Deploy(int count = -1)
        {
            Ped player = Game.Player.Character;
            if (!Utils.AlivePed(player))
            {
                Utils.Notify("Su anda koruma cagrilamaz.");
                return;
            }

            if (count > 0) TargetCount = Math.Min(count, _cfg.MaxGuards);
            Active = true;

            int spawned = 0;
            int needed = TargetCount - LivingCount();
            for (int i = 0; i < needed; i++)
            {
                if (SpawnGuard(player) != null) spawned++;
            }

            if (_guards.Count == 0)
            {
                Active = false;
                Utils.Notify("Koruma spawn edilemedi. Model isimlerini kontrol edin.");
                return;
            }

            AssignSlots();
            Utils.Notify(string.Format("Yakin koruma goreve hazir ({0} birim).", _guards.Count));
            Logger.Info("CloseProtection.Deploy: " + spawned + " koruma spawn edildi.");
        }

        public void Dismiss(bool delete = true)
        {
            for (int i = 0; i < _guards.Count; i++)
            {
                Guard guard = _guards[i];
                Utils.RemoveBlip(guard.Blip);

                if (!Utils.Valid(guard.Ped)) continue;

                if (delete)
                {
                    Utils.DeleteEntity(guard.Ped);
                }
                else
                {
                    N.ClearTasks(guard.Ped.Handle);
                    N.TaskWander(guard.Ped.Handle);
                    Utils.Release(guard.Ped);
                }
            }

            _guards.Clear();
            _respawnQueue.Clear();
            _pedView.Clear();
            Active = false;
            Utils.Notify("Yakin koruma ekibi gorevden alindi.");
        }

        public void Toggle()
        {
            if (Active) Dismiss();
            else Deploy();
        }

        /// <summary>Disaridan gelen (yedek birlik vb.) bir ped'i ekibe katar.</summary>
        public bool Adopt(Ped ped)
        {
            if (!Utils.AlivePed(ped)) return false;
            if (LivingCount() >= _cfg.MaxGuards) return false;

            for (int i = 0; i < _guards.Count; i++)
                if (_guards[i].Ped != null && _guards[i].Ped.Handle == ped.Handle) return false;

            Guard guard = new Guard { Ped = ped };
            AttachBlip(guard);
            _guards.Add(guard);
            Active = true;
            AssignSlots();
            return true;
        }

        public void SetCount(int count)
        {
            TargetCount = Math.Max(1, Math.Min(count, _cfg.MaxGuards));

            if (!Active) return;

            // Fazlaliklari gonder
            while (LivingCount() > TargetCount && _guards.Count > 0)
            {
                Guard guard = _guards[_guards.Count - 1];
                Utils.RemoveBlip(guard.Blip);
                Utils.DeleteEntity(guard.Ped);
                _guards.RemoveAt(_guards.Count - 1);
            }

            // Eksikleri tamamla
            Ped player = Game.Player.Character;
            while (LivingCount() < TargetCount && Utils.AlivePed(player))
            {
                if (SpawnGuard(player) == null) break;
            }

            AssignSlots();
            Utils.Notify("Koruma sayisi: " + _guards.Count + "/" + TargetCount);
        }

        public void SetFormation(Formation formation)
        {
            Formation = formation;
            AssignSlots();
            ForceRetask();
            Utils.Notify("Formasyon: " + FormationName(formation));
        }

        public void CycleFormation()
        {
            Array values = Enum.GetValues(typeof(Formation));
            int index = Array.IndexOf(values, Formation);
            SetFormation((Formation)values.GetValue((index + 1) % values.Length));
        }

        public void SetBehavior(GuardBehavior behavior)
        {
            Behavior = behavior;
            ForceRetask();

            if (behavior == GuardBehavior.Passive)
            {
                for (int i = 0; i < _guards.Count; i++)
                {
                    if (Utils.AlivePed(_guards[i].Ped))
                        N.ClearTasks(_guards[i].Ped.Handle);
                    _guards[i].TargetHandle = 0;
                }
            }

            Utils.Notify("Davranis: " + BehaviorName(behavior));
        }

        /// <summary>Tum ekibin ana silahini degistirir. null = ayardan rastgele.</summary>
        public void SetWeapon(string weaponName)
        {
            WeaponOverride = weaponName;

            for (int i = 0; i < _guards.Count; i++)
            {
                Guard guard = _guards[i];
                if (!Utils.AlivePed(guard.Ped)) continue;

                try { guard.Ped.Weapons.RemoveAll(); }
                catch { /* onemli degil */ }

                GuardFactory.GiveWeapons(guard.Ped, _cfg, weaponName);
            }

            Utils.Notify("Silah: " + (string.IsNullOrEmpty(weaponName) ? "Rastgele" : weaponName));
        }

        // ==================================================================
        // Ana dongu
        // ==================================================================
        public void Update()
        {
            if (!Active) return;

            Ped player = Game.Player.Character;
            if (!Utils.Valid(player)) return;

            HandleDeaths();
            HandleRespawns(player);

            if (_guards.Count == 0 && _respawnQueue.Count == 0)
            {
                Active = false;
                return;
            }

            // --- Oyuncu olduyse ---
            if (player.IsDead)
            {
                if (!_playerWasDead)
                {
                    _playerWasDead = true;
                    ApplyDeathBehavior(player);
                }
                return;
            }

            if (_playerWasDead)
            {
                _playerWasDead = false;
                Regroup(player);
            }

            DetectCodeRed(player);

            bool playerInVehicle = player.IsInVehicle();
            _reservedSeats.Clear();

            GuardBehavior behavior = EffectiveBehavior;

            // Hedefleri paylastir: tum korumalar bagimsizca "en yakini" secerse
            // hepsi ayni dusmana uşuşur, digerleri acikta kalir.
            Dictionary<int, Ped> assignment = (behavior != GuardBehavior.Passive && _scanner.HasThreats)
                ? _scanner.AssignTargets(Peds)
                : null;

            for (int i = 0; i < _guards.Count; i++)
            {
                Guard guard = _guards[i];
                if (!Utils.AlivePed(guard.Ped)) continue;

                try
                {
                    if (playerInVehicle) UpdateGuardInVehicleMode(guard, player, behavior, assignment);
                    else UpdateGuardOnFootMode(guard, player, behavior, assignment);
                }
                catch (Exception ex)
                {
                    Logger.Error("Koruma guncelleme hatasi", ex);
                }
            }
        }

        /// <summary>Oyuncu kisa surede buyuk can kaybi yasarsa tum ekip gecici olarak agresiflesir.</summary>
        private void DetectCodeRed(Ped player)
        {
            if (!_cfg.CodeRedEnabled) return;

            int currentHealth = player.Health;
            if (_lastPlayerHealth >= 0)
            {
                int drop = _lastPlayerHealth - currentHealth;
                if (drop >= _cfg.CodeRedHealthDropThreshold && Game.GameTime >= _codeRedUntil)
                {
                    _codeRedUntil = Game.GameTime + _cfg.CodeRedDuration;
                    Utils.Notify("~r~KOD KIRMIZI~s~ - ekip tam alarm durumunda!");
                    Logger.Info("Kod Kirmizi tetiklendi (can dususu: " + drop + ").");
                }
            }
            _lastPlayerHealth = currentHealth;
        }

        // ------------------------------------------------------------------
        // Yaya modu
        // ------------------------------------------------------------------
        private void UpdateGuardOnFootMode(Guard guard, Ped player, GuardBehavior behavior, Dictionary<int, Ped> assignment)
        {
            Ped ped = guard.Ped;
            int now = Game.GameTime;

            // Aracin icindeyse once insin.
            if (ped.IsInVehicle())
            {
                if (guard.State != GuardState.Boarding || now - guard.BoardingSince > 1500)
                {
                    Vehicle vehicle = ped.CurrentVehicle;
                    if (Utils.Valid(vehicle))
                        N.TaskLeaveVehicle(ped.Handle, vehicle.Handle, 0);

                    guard.State = GuardState.Idle;
                    guard.SeatIndex = -99;
                    guard.LastTaskTime = now;
                }
                return;
            }

            guard.SeatIndex = -99;

            // Cok geride kaldiysa isinla (oyuncunun arkasina).
            float distance = ped.Position.DistanceTo(player.Position);
            if (distance > _cfg.TeleportDistance)
            {
                Vector3 target = Utils.SafeGround(Utils.OffsetOf(player, new Vector3(
                    (float)(Utils.Rng.NextDouble() * 4 - 2), -4f - (float)Utils.Rng.NextDouble() * 2, 0f)));
                ped.Position = target;
                guard.State = GuardState.Idle;
                guard.LastTaskTime = 0;
                return;
            }

            // --- Savas ---
            if (behavior != GuardBehavior.Passive && assignment != null)
            {
                Ped target;
                if (assignment.TryGetValue(ped.Handle, out target) && Utils.AlivePed(target))
                {
                    if (guard.TargetHandle != target.Handle || guard.State != GuardState.Fighting || now - guard.LastTaskTime > 6000)
                    {
                        N.TaskCombatPed(ped.Handle, target.Handle);
                        guard.TargetHandle = target.Handle;
                        guard.State = GuardState.Fighting;
                        guard.LastTaskTime = now;
                    }
                    return;
                }
            }

            // --- Formasyon ---
            bool needsRetask = guard.State != GuardState.Following || now - guard.LastTaskTime > 7000;

            if (guard.State == GuardState.Fighting)
            {
                // Catisma bitti, formasyona don.
                guard.TargetHandle = 0;
                needsRetask = true;
            }

            if (needsRetask)
            {
                Vector3 offset = SlotOffset(Formation, guard.SlotIndex, Math.Max(_guards.Count, 1));

                // Oyuncunun gercek hareket durumuna gore koru: yururken yuru,
                // kosarken kos, sprint atarken sprint. Sabit bir hiz kullanmak
                // korumalarin oyuncuyu kaybetmesine ya da onunde takilmasina
                // sebep oluyordu ("takip sistemi bozuk" sikayeti).
                float followSpeed = player.IsSprinting ? 3f : player.IsRunning ? 2f : _cfg.GuardFollowSpeed;

                N.TaskFollowToOffsetOfEntity(
                    ped.Handle, player.Handle, offset,
                    followSpeed, -1, _cfg.GuardStoppingRange, true);

                guard.State = GuardState.Following;
                guard.LastTaskTime = now;
            }
        }

        // ------------------------------------------------------------------
        // Arac modu
        // ------------------------------------------------------------------
        private void UpdateGuardInVehicleMode(Guard guard, Ped player, GuardBehavior behavior, Dictionary<int, Ped> assignment)
        {
            Ped ped = guard.Ped;
            Vehicle playerVehicle = player.CurrentVehicle;
            int now = Game.GameTime;

            if (!Utils.Valid(playerVehicle)) return;

            // Zaten bir aracin icinde mi?
            if (ped.IsInVehicle())
            {
                guard.State = GuardState.InVehicle;

                // Aractan drive-by ile karsilik ver.
                if (behavior != GuardBehavior.Passive && assignment != null)
                {
                    Ped target;
                    if (assignment.TryGetValue(ped.Handle, out target) && Utils.AlivePed(target) &&
                        (guard.TargetHandle != target.Handle || now - guard.LastTaskTime > 5000))
                    {
                        N.TaskCombatPed(ped.Handle, target.Handle);
                        guard.TargetHandle = target.Handle;
                        guard.LastTaskTime = now;
                    }
                }
                return;
            }

            // Bos koltuk bul: once oyuncunun araci, sonra konvoy destek araci.
            Vehicle targetVehicle = playerVehicle;
            int seat = FindFreeSeat(targetVehicle);

            if (seat == int.MinValue && SupportVehicleProvider != null)
            {
                Vehicle support = SupportVehicleProvider();
                if (Utils.DrivableVehicle(support))
                {
                    int supportSeat = FindFreeSeat(support);
                    if (supportSeat != int.MinValue)
                    {
                        targetVehicle = support;
                        seat = supportSeat;
                    }
                }
            }

            if (seat == int.MinValue)
            {
                // Yer yok: oyuncuyu yaya olarak takip etsin.
                if (guard.State != GuardState.Following || now - guard.LastTaskTime > 5000)
                {
                    Vector3 offset = SlotOffset(Formation, guard.SlotIndex, Math.Max(_guards.Count, 1));
                    N.TaskFollowToOffsetOfEntity(ped.Handle, player.Handle, offset, 3f, -1, 4f, true);
                    guard.State = GuardState.Following;
                    guard.LastTaskTime = now;
                }
                return;
            }

            _reservedSeats.Add(SeatKey(targetVehicle, seat));

            float distance = ped.Position.DistanceTo(targetVehicle.Position);

            // Uzaksa ya da arac hareket halindeyse isinlayarak bindir (takilma olmasin).
            if (distance > 25f || targetVehicle.Speed > 6f)
            {
                N.SetIntoVehicle(ped.Handle, targetVehicle.Handle, seat);
                guard.State = GuardState.InVehicle;
                guard.SeatIndex = seat;
                guard.LastTaskTime = now;
                return;
            }

            if (guard.State != GuardState.Boarding || now - guard.BoardingSince > 6000)
            {
                N.TaskEnterVehicle(ped.Handle, targetVehicle.Handle, 15000, seat, 2f, 1);
                guard.State = GuardState.Boarding;
                guard.SeatIndex = seat;
                guard.BoardingSince = now;
                guard.LastTaskTime = now;
            }
        }

        private int SeatKey(Vehicle vehicle, int seat)
        {
            return unchecked(vehicle.Handle * 397 + seat);
        }

        /// <summary>Bos yolcu koltugu bulur; yoksa int.MinValue.</summary>
        private int FindFreeSeat(Vehicle vehicle)
        {
            if (!Utils.DrivableVehicle(vehicle)) return int.MinValue;

            int maxPassengers = N.GetMaxPassengers(vehicle.Handle);
            for (int seat = 0; seat < maxPassengers; seat++)
            {
                if (_reservedSeats.Contains(SeatKey(vehicle, seat))) continue;
                if (N.IsSeatFree(vehicle.Handle, seat)) return seat;
            }
            return int.MinValue;
        }

        // ------------------------------------------------------------------
        // Olum / yeniden dogum
        // ------------------------------------------------------------------
        private void HandleDeaths()
        {
            int now = Game.GameTime;

            for (int i = _guards.Count - 1; i >= 0; i--)
            {
                Guard guard = _guards[i];

                if (Utils.AlivePed(guard.Ped)) continue;

                // --- Olum ilk kez tespit edildi ---
                if (guard.DeathTime == 0)
                {
                    guard.DeathTime = now;
                    Utils.RemoveBlip(guard.Blip);
                    guard.Blip = null;

                    // Yedegi hemen kuyruga al (ceset temizligini beklemeden).
                    if (_cfg.GuardAutoRespawn && Active && LivingCount() + _respawnQueue.Count < TargetCount)
                    {
                        _respawnQueue.Add(now + _cfg.GuardRespawnDelay);
                        Logger.Info("Koruma dustu, yedek " + (_cfg.GuardRespawnDelay / 1000) + " sn icinde geliyor.");
                    }
                }

                // Ceset hala sahnedeyse temizlik suresini bekle.
                if (Utils.Valid(guard.Ped))
                {
                    if (_cfg.RemoveDeadBodies)
                    {
                        if (now - guard.DeathTime < _cfg.DeadBodyCleanupDelay) continue;
                        Utils.DeleteEntity(guard.Ped);
                    }
                    else
                    {
                        Utils.Release(guard.Ped);
                    }
                }

                _guards.RemoveAt(i);
                AssignSlots();
            }
        }

        private void HandleRespawns(Ped player)
        {
            if (_respawnQueue.Count == 0) return;
            if (!Utils.AlivePed(player)) return;

            int now = Game.GameTime;
            for (int i = _respawnQueue.Count - 1; i >= 0; i--)
            {
                if (_respawnQueue[i] > now) continue;
                _respawnQueue.RemoveAt(i);

                if (LivingCount() >= TargetCount) continue;

                Ped replacement = SpawnGuard(player);
                if (replacement != null)
                {
                    AssignSlots();
                    Utils.Notify("Yedek koruma goreve katildi.");
                }
            }
        }

        private void ApplyDeathBehavior(Ped player)
        {
            switch (_cfg.PlayerDeath)
            {
                case DeathBehavior.Vanish:
                    Dismiss();
                    break;

                case DeathBehavior.Scatter:
                    for (int i = 0; i < _guards.Count; i++)
                    {
                        Guard guard = _guards[i];
                        if (!Utils.AlivePed(guard.Ped)) continue;
                        N.ClearTasks(guard.Ped.Handle);
                        N.TaskSmartFlee(guard.Ped.Handle, player.Handle, 150f, -1);
                        guard.State = GuardState.Mourning;
                    }
                    break;

                default: // Guard
                    for (int i = 0; i < _guards.Count; i++)
                    {
                        Guard guard = _guards[i];
                        if (!Utils.AlivePed(guard.Ped)) continue;

                        Vector3 spot = Utils.PointOnCircle(player.Position, 3.5f, i, Math.Max(_guards.Count, 1));
                        spot = Utils.SafeGround(spot);
                        N.ClearTasks(guard.Ped.Handle);
                        N.TaskStandGuard(guard.Ped.Handle, spot,
                            Utils.HeadingTowards(spot, player.Position), "WORLD_HUMAN_GUARD_STAND");
                        guard.State = GuardState.Mourning;
                    }
                    break;
            }
        }

        /// <summary>Oyuncu yeniden dogduktan sonra ekibi topla.</summary>
        private void Regroup(Ped player)
        {
            for (int i = 0; i < _guards.Count; i++)
            {
                Guard guard = _guards[i];
                if (!Utils.AlivePed(guard.Ped)) continue;

                if (guard.Ped.Position.DistanceTo(player.Position) > 60f)
                {
                    Vector3 spot = Utils.SafeGround(Utils.OffsetOf(player, new Vector3(
                        (float)(Utils.Rng.NextDouble() * 4 - 2), -4f, 0f)));
                    guard.Ped.Position = spot;
                }

                N.ClearTasks(guard.Ped.Handle);
                guard.State = GuardState.Idle;
                guard.LastTaskTime = 0;
            }

            ForceRetask();
        }

        // ------------------------------------------------------------------
        // Spawn ve slotlar
        // ------------------------------------------------------------------
        private Ped SpawnGuard(Ped player)
        {
            if (LivingCount() >= _cfg.MaxGuards) return null;

            // Oyuncunun arkasinda, gorus alani disinda bir nokta.
            float side = (float)(Utils.Rng.NextDouble() * 5.0 - 2.5);
            Vector3 spawnPoint = Utils.OffsetOf(player, new Vector3(side, -5f - (float)Utils.Rng.NextDouble() * 3f, 0f));
            spawnPoint = Utils.SafeGround(spawnPoint);

            Ped ped = GuardFactory.CreateGuard(_cfg, spawnPoint, player.Heading, WeaponOverride);
            if (ped == null) return null;

            Guard guard = new Guard { Ped = ped, SlotIndex = _guards.Count, LastTaskTime = 0 };
            AttachBlip(guard);
            _guards.Add(guard);
            return ped;
        }

        private void AttachBlip(Guard guard)
        {
            if (!_cfg.EnableBlips || !Utils.Valid(guard.Ped)) return;

            Utils.RemoveBlip(guard.Blip);
            guard.Blip = Utils.AttachBlip(guard.Ped, _cfg.GuardBlipSprite, _cfg.GuardBlipColor, "Koruma", 0.65f);
        }

        private void AssignSlots()
        {
            for (int i = 0; i < _guards.Count; i++)
                _guards[i].SlotIndex = i;
        }

        private void ForceRetask()
        {
            for (int i = 0; i < _guards.Count; i++)
            {
                _guards[i].LastTaskTime = 0;
                if (_guards[i].State == GuardState.Following)
                    _guards[i].State = GuardState.Idle;
            }
        }

        // ------------------------------------------------------------------
        // Formasyon geometrisi (oyuncuya gore lokal offset: X sag, Y ileri)
        // ------------------------------------------------------------------
        private static readonly Vector3[] DiamondSlots =
        {
            new Vector3(-1.7f,  1.6f, 0f),   // sol on
            new Vector3( 1.7f,  1.6f, 0f),   // sag on
            new Vector3(-2.2f, -1.4f, 0f),   // sol arka
            new Vector3( 2.2f, -1.4f, 0f),   // sag arka
            new Vector3( 0.0f, -2.6f, 0f),   // arka orta
            new Vector3(-2.8f,  0.0f, 0f),   // sol yan
            new Vector3( 2.8f,  0.0f, 0f),   // sag yan
            new Vector3( 0.0f,  2.8f, 0f)    // on orta
        };

        private static readonly Vector3[] BoxSlots =
        {
            new Vector3(-2.4f,  2.4f, 0f),
            new Vector3( 2.4f,  2.4f, 0f),
            new Vector3(-2.4f, -2.4f, 0f),
            new Vector3( 2.4f, -2.4f, 0f),
            new Vector3( 0.0f,  3.0f, 0f),
            new Vector3( 0.0f, -3.0f, 0f),
            new Vector3(-3.0f,  0.0f, 0f),
            new Vector3( 3.0f,  0.0f, 0f)
        };

        private static Vector3 SlotOffset(Formation formation, int index, int total)
        {
            if (index < 0) index = 0;

            switch (formation)
            {
                case Formation.Box:
                    return BoxSlots[index % BoxSlots.Length];

                case Formation.Column:
                    // Tek sira: arkada dizilirler.
                    return new Vector3(index % 2 == 0 ? -0.7f : 0.7f, -1.8f - 1.6f * (index / 2), 0f);

                case Formation.Wedge:
                    {
                        int rank = index / 2 + 1;
                        float side = index % 2 == 0 ? -1f : 1f;
                        return new Vector3(side * 1.5f * rank, -1.2f * rank, 0f);
                    }

                case Formation.Circle:
                    {
                        int count = Math.Max(total, 1);
                        double angle = (2.0 * Math.PI / count) * index;
                        const float radius = 3.2f;
                        return new Vector3((float)Math.Cos(angle) * radius, (float)Math.Sin(angle) * radius, 0f);
                    }

                default:
                    return DiamondSlots[index % DiamondSlots.Length];
            }
        }

        public static string FormationName(Formation formation)
        {
            switch (formation)
            {
                case Formation.Box: return "Kutu";
                case Formation.Column: return "Kolon";
                case Formation.Wedge: return "Kama";
                case Formation.Circle: return "Cember (360)";
                default: return "Elmas";
            }
        }

        public static string BehaviorName(GuardBehavior behavior)
        {
            switch (behavior)
            {
                case GuardBehavior.Aggressive: return "Agresif";
                case GuardBehavior.Passive: return "Pasif";
                default: return "Defansif";
            }
        }

        // ------------------------------------------------------------------
        // Temizlik
        // ------------------------------------------------------------------
        public void CleanupAll()
        {
            for (int i = 0; i < _guards.Count; i++)
            {
                Utils.RemoveBlip(_guards[i].Blip);
                Utils.DeleteEntity(_guards[i].Ped);
            }
            _guards.Clear();
            _respawnQueue.Clear();
            _pedView.Clear();
            Active = false;
        }
    }
}
