using System;
using System.Collections.Generic;
using GTA;
using GTA.Math;

namespace MafiaVIP
{
    /// <summary>
    /// Tehdit tespit motoru.
    ///
    /// Mantik: bir ped ancak "dusmanca bir eylem" yaptiginda tehdit listesine
    /// girer (oyuncuya/ekibe ates etti, savas moduna girdi, hasar verdi).
    /// Bir kez listeye giren hedef, belirli bir sure boyunca "aggro" kalir;
    /// boylece korumalar hedef siper alip ates kestiginde takibi birakmaz.
    ///
    /// Bu tasarim sayesinde korumalar sivillere / polise rastgele saldirmaz.
    /// </summary>
    internal sealed class ThreatScanner
    {
        private const int AggroDuration = 20000;   // ms, tehdit hafizasi
        private const int PedTypeCop = 6;
        private const int PedTypeSwat = 27;
        private const int PedTypeArmy = 29;

        private readonly Config _cfg;

        /// <summary>Ped handle -> aggro bitis zamani (GameTime).</summary>
        private readonly Dictionary<int, int> _aggro = new Dictionary<int, int>();
        private readonly Dictionary<int, Ped> _pedCache = new Dictionary<int, Ped>();
        private readonly List<Ped> _threats = new List<Ped>();
        private readonly List<int> _expired = new List<int>();

        public ThreatScanner(Config cfg)
        {
            _cfg = cfg;
        }

        public IList<Ped> Threats { get { return _threats; } }
        public bool HasThreats { get { return _threats.Count > 0; } }

        /// <summary>Disaridan zorla tehdit ekleme (ornegin oyuncu hedef isaretledi).</summary>
        public void Mark(Ped ped)
        {
            if (!Utils.AlivePed(ped)) return;
            if (Relationships.IsFriendly(ped)) return;

            _aggro[ped.Handle] = Game.GameTime + AggroDuration;
            _pedCache[ped.Handle] = ped;
        }

        public void Clear()
        {
            _aggro.Clear();
            _pedCache.Clear();
            _threats.Clear();
        }

        /// <summary>
        /// Cevreyi tarar ve tehdit listesini gunceller.
        /// Pahali bir islem oldugu icin cagiran taraf ~750 ms araliklarla cagirmali.
        /// </summary>
        public void Update(Ped player, IList<Ped> friendlies, GuardBehavior behavior)
        {
            _threats.Clear();
            if (!Utils.AlivePed(player)) { _aggro.Clear(); return; }
            if (behavior == GuardBehavior.Passive) { _aggro.Clear(); return; }

            int now = Game.GameTime;

            try
            {
                Ped[] nearby = World.GetNearbyPeds(player, _cfg.ThreatRadius);
                bool wanted = Game.Player.WantedLevel > 0;

                foreach (Ped ped in nearby)
                {
                    if (!Utils.AlivePed(ped)) continue;
                    if (ped.Handle == player.Handle) continue;
                    if (ped.IsPlayer) continue;
                    if (Relationships.IsFriendly(ped)) continue;

                    if (IsHostile(ped, player, friendlies, behavior, wanted))
                    {
                        _aggro[ped.Handle] = now + AggroDuration;
                        _pedCache[ped.Handle] = ped;
                    }
                }

                // Oyuncunun son hasar kaynagini temizle ki tekrar tetiklensin.
                N.ClearLastDamageEntity(player.Handle);
            }
            catch (Exception ex)
            {
                Logger.Error("ThreatScanner.Update tarama hatasi", ex);
            }

            // Aggro listesini suzup gecerli tehditleri cikar.
            _expired.Clear();
            float maxDistance = _cfg.ThreatRadius * 2f;

            foreach (KeyValuePair<int, int> entry in _aggro)
            {
                Ped ped;
                if (!_pedCache.TryGetValue(entry.Key, out ped) || !Utils.AlivePed(ped))
                {
                    _expired.Add(entry.Key);
                    continue;
                }

                if (entry.Value <= now)
                {
                    _expired.Add(entry.Key);
                    continue;
                }

                if (ped.Position.DistanceTo(player.Position) > maxDistance)
                {
                    _expired.Add(entry.Key);
                    continue;
                }

                _threats.Add(ped);
            }

            foreach (int handle in _expired)
            {
                _aggro.Remove(handle);
                _pedCache.Remove(handle);
            }
        }

        private bool IsHostile(Ped ped, Ped player, IList<Ped> friendlies, GuardBehavior behavior, bool wanted)
        {
            int h = ped.Handle;

            // 1) Oyuncuyla acik catisma / oyuncuya hasar
            if (N.IsPedInCombatWith(h, player.Handle)) return true;
            if (N.HasBeenDamagedBy(player.Handle, h)) return true;

            // 2) Ekibimizden birine saldiriyor mu?
            if (friendlies != null)
            {
                for (int i = 0; i < friendlies.Count; i++)
                {
                    Ped friend = friendlies[i];
                    if (!Utils.AlivePed(friend)) continue;

                    if (N.IsPedInCombatWith(h, friend.Handle)) return true;
                    if (N.HasBeenDamagedBy(friend.Handle, h))
                    {
                        N.ClearLastDamageEntity(friend.Handle);
                        return true;
                    }
                }
            }

            // 3) Polis / SWAT / ordu -> yalnizca ayar aciksa ve aranma varsa
            if (_cfg.EngagePolice && wanted)
            {
                int type = N.GetPedType(h);
                if (type == PedTypeCop || type == PedTypeSwat || type == PedTypeArmy)
                    return true;
            }

            // 4) Agresif modda: yakinda silahli ve ates eden herkes tehdittir
            if (behavior == GuardBehavior.Aggressive)
            {
                if (N.IsPedShooting(h) &&
                    ped.Position.DistanceTo(player.Position) <= _cfg.ThreatRadius * 0.8f)
                    return true;
            }

            return false;
        }

        /// <summary>Verilen noktaya en yakin canli tehdit; yoksa null.</summary>
        public Ped Nearest(Vector3 from)
        {
            Ped best = null;
            float bestDistance = float.MaxValue;

            for (int i = 0; i < _threats.Count; i++)
            {
                Ped ped = _threats[i];
                if (!Utils.AlivePed(ped)) continue;

                float distance = ped.Position.DistanceTo(from);
                if (distance < bestDistance)
                {
                    bestDistance = distance;
                    best = ped;
                }
            }

            return best;
        }
    }
}
