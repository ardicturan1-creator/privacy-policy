using System;
using GTA;

namespace MafiaVIP
{
    /// <summary>
    /// Koruma ekibinin iliski grubunu yonetir.
    ///
    /// Amac: korumalar oyuncuya ve birbirlerine "companion" olsun; sivil ve
    /// polise karsi NOTR kalsin. Boylece kimseye rastgele saldirmazlar,
    /// yalnizca ThreatScanner tarafindan verilen hedeflerle savasirlar.
    ///
    /// Iliski degerleri: 0 Companion, 1 Respect, 2 Like, 3 Neutral, 4 Dislike, 5 Hate
    /// </summary>
    internal static class Relationships
    {
        public const int Companion = 0;
        public const int Respect = 1;
        public const int Like = 2;
        public const int Neutral = 3;
        public const int Dislike = 4;
        public const int Hate = 5;

        public static int GuardGroup { get; private set; }
        public static bool Ready { get; private set; }

        private static readonly string[] NeutralGroups =
        {
            "CIVMALE", "CIVFEMALE", "COP", "SECURITY_GUARD", "PRIVATE_SECURITY",
            "FIREMAN", "MEDIC", "PLAYER"
        };

        public static void Setup(string groupName)
        {
            try
            {
                if (string.IsNullOrEmpty(groupName))
                    groupName = "MAFIA_VIP_GUARDS";

                N.AddRelationshipGroup(groupName);
                GuardGroup = Game.GenerateHash(groupName);

                int player = Game.GenerateHash("PLAYER");

                // Ekip ici ve oyuncuyla dost
                Both(Companion, GuardGroup, GuardGroup);
                Both(Companion, GuardGroup, player);

                // Sivil / polis / acil servis: notr (kendiliginden saldirmazlar)
                foreach (string name in NeutralGroups)
                {
                    int hash = Game.GenerateHash(name);
                    if (hash == player || hash == GuardGroup) continue;
                    Both(Neutral, GuardGroup, hash);
                }

                Ready = true;
                Logger.Info("Iliski grubu hazir: " + groupName);
            }
            catch (Exception ex)
            {
                Ready = false;
                Logger.Error("Relationships.Setup hatasi", ex);
            }
        }

        /// <summary>Iki yonlu iliski tanimi.</summary>
        private static void Both(int relation, int a, int b)
        {
            N.SetRelationshipBetweenGroups(relation, a, b);
            N.SetRelationshipBetweenGroups(relation, b, a);
        }

        /// <summary>Verilen ped'i koruma grubuna dahil eder.</summary>
        public static void Apply(Ped ped)
        {
            if (!Utils.Valid(ped)) return;
            if (!Ready) Setup("MAFIA_VIP_GUARDS");

            N.SetRelationshipGroup(ped.Handle, GuardGroup);
            N.SetCanAttackFriendly(ped.Handle, false, false);
        }

        /// <summary>Ped bizim ekipten mi?</summary>
        public static bool IsFriendly(Ped ped)
        {
            if (!Utils.Valid(ped)) return false;
            try { return N.GetRelationshipGroup(ped.Handle) == GuardGroup; }
            catch { return false; }
        }
    }
}
