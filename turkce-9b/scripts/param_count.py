#!/usr/bin/env python3
"""tr-sohbet-9b parametre sayaci.

Mimarinin gercekten 9B'ye oturdugunu dogrular. Formuller Llama-tarzi
(pre-RMSNorm + GQA + SwiGLU + RoPE, bias'siz) bir bloga gore yazilmistir.
"""
import argparse
import json
import pathlib


def count(cfg: dict) -> dict:
    d = cfg["d_model"]
    L = cfg["n_layers"]
    hd = cfg["head_dim"]
    kv = cfg["n_kv_heads"] * hd
    q = cfg["n_heads"] * hd
    h = cfg["ffn_hidden"]
    V = cfg["vocab_size"]

    assert q == d, f"n_heads*head_dim ({q}) d_model ({d}) ile esit olmali"

    # Dikkat: Wq (d x q), Wk/Wv (d x kv), Wo (q x d)
    attn = d * q + 2 * (d * kv) + q * d
    # SwiGLU: gate + up + down
    ffn = 3 * d * h
    # Blok basina 2 RMSNorm agirligi
    norms = 2 * d
    # QK-norm kullaniliyorsa head_dim boyutunda iki olcek daha
    if cfg.get("qk_norm"):
        norms += 2 * hd

    per_layer = attn + ffn + norms
    blocks = per_layer * L
    embed = V * d
    out_head = 0 if cfg.get("tie_word_embeddings") else V * d
    final_norm = d

    total = blocks + embed + out_head + final_norm
    return {
        "attn_per_layer": attn,
        "ffn_per_layer": ffn,
        "per_layer": per_layer,
        "blocks_total": blocks,
        "embedding": embed,
        "output_head": out_head,
        "final_norm": final_norm,
        "total": total,
        "non_embedding": total - embed - out_head,
    }


def main() -> None:
    ap = argparse.ArgumentParser()
    default = pathlib.Path(__file__).resolve().parents[1] / "configs" / "model_9b.json"
    ap.add_argument("--config", default=str(default))
    args = ap.parse_args()

    cfg = json.loads(pathlib.Path(args.config).read_text())
    r = count(cfg)
    for k, v in r.items():
        print(f"{k:>18}: {v:>15,}  ({v/1e9:.3f}B)")

    # Egitim FLOP tahmini: ~6 * N * D (ileri+geri)
    for tokens in (180e9, 300e9, 450e9):
        flops = 6 * r["non_embedding"] * tokens
        # H100 bf16 tepe 989 TFLOPs, %40 MFU varsayimi
        gpu_h = flops / (989e12 * 0.40) / 3600
        print(f"  {tokens/1e9:.0f}B token -> {flops:.3e} FLOP, ~{gpu_h:,.0f} H100-saat")


if __name__ == "__main__":
    main()
