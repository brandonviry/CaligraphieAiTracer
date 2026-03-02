#!/usr/bin/env python3
"""
send_job.py — Envoie un ou plusieurs jobs de dessin à CaligraphieAiTracer.

Usage :
    python send_job.py                        # envoie le job par défaut
    python send_job.py "Bonjour" Pacifico     # texte + police
    python send_job.py --demo                 # séquence de démonstration

Prérequis :
    - CaligraphieAiTracer doit être lancé (cargo run ou .exe)
    - Python 3.6+, aucun module externe requis
"""

import socket
import json
import time
import argparse
import sys

HOST = "127.0.0.1"
PORT = 7777


def send_job(job: dict, host: str = HOST, port: int = PORT) -> bool:
    """Envoie un job JSON à l'application. Retourne True si succès."""
    try:
        with socket.create_connection((host, port), timeout=5) as s:
            payload = json.dumps(job, ensure_ascii=False) + "\n"
            s.sendall(payload.encode("utf-8"))
            print(f"  ✓ Envoyé : {payload.strip()}")
            return True
    except ConnectionRefusedError:
        print(f"  ✗ Connexion refusée sur {host}:{port}")
        print("    → L'application CaligraphieAiTracer est-elle lancée ?")
        return False
    except Exception as e:
        print(f"  ✗ Erreur : {e}")
        return False


def demo_sequence():
    """Envoie une séquence de jobs de démonstration."""
    jobs = [
        {
            "text": "Bonjour",
            "font": "Dancing Script",
            "brush": "brush_pen",
            "color": [10, 10, 30],
            "thickness": 20.0,
            "speed": 6.0,
            "clear_before": True,
        },
        {
            "text": "Hello",
            "font": "Great Vibes",
            "brush": "round_smooth",
            "color": [180, 20, 20],
            "thickness": 16.0,
            "speed": 8.0,
            "clear_before": True,
        },
        {
            "text": "Calligraphie",
            "font": "Pacifico",
            "brush": "flat_calligraphy",
            "color": [20, 80, 140],
            "thickness": 22.0,
            "speed": 5.0,
            "clear_before": True,
        },
        {
            "text": "Art",
            "font": "Sacramento",
            "brush": "dry_ink",
            "color": [40, 40, 40],
            "thickness": 28.0,
            "speed": 4.0,
            "clear_before": True,
        },
    ]

    print(f"\n=== Démonstration : {len(jobs)} jobs ===\n")
    for i, job in enumerate(jobs, 1):
        print(f"Job {i}/{len(jobs)} : «{job['text']}» ({job['font']})")
        if not send_job(job):
            print("Arrêt de la démonstration.")
            break
        # Attendre que le dessin se termine avant le suivant
        # (estimation : 3 sec par job, ajuster selon la vitesse)
        if i < len(jobs):
            wait = 4.0
            print(f"  → Attente {wait}s...\n")
            time.sleep(wait)

    print("\n=== Démonstration terminée ===")


def main():
    parser = argparse.ArgumentParser(
        description="Envoie un job de dessin à CaligraphieAiTracer"
    )
    parser.add_argument("text", nargs="?", default="Bonjour",
                        help="Texte à tracer (défaut: 'Bonjour')")
    parser.add_argument("font", nargs="?", default=None,
                        help="Police Google Fonts (ex: 'Dancing Script')")
    parser.add_argument("--brush", default=None,
                        help="Brosse: round_smooth, flat_calligraphy, brush_pen, dry_ink")
    parser.add_argument("--color", nargs=3, type=int, metavar=("R", "G", "B"),
                        default=None, help="Couleur encre RGB (ex: 10 10 30)")
    parser.add_argument("--thickness", type=float, default=None,
                        help="Épaisseur de base en pixels")
    parser.add_argument("--speed", type=float, default=None,
                        help="Vitesse d'animation (points/frame)")
    parser.add_argument("--no-clear", action="store_true",
                        help="Ne pas effacer le canvas avant de tracer")
    parser.add_argument("--host", default=HOST,
                        help=f"Adresse de l'app (défaut: {HOST})")
    parser.add_argument("--port", type=int, default=PORT,
                        help=f"Port TCP (défaut: {PORT})")
    parser.add_argument("--demo", action="store_true",
                        help="Lance la séquence de démonstration")

    args = parser.parse_args()

    if args.demo:
        demo_sequence()
        return

    # Job simple
    job = {"text": args.text, "clear_before": not args.no_clear}

    if args.font:      job["font"]      = args.font
    if args.brush:     job["brush"]     = args.brush
    if args.color:     job["color"]     = args.color
    if args.thickness: job["thickness"] = args.thickness
    if args.speed:     job["speed"]     = args.speed

    print(f"\nEnvoi du job à {args.host}:{args.port}")
    success = send_job(job, args.host, args.port)
    sys.exit(0 if success else 1)


if __name__ == "__main__":
    main()
