# Configuration Nginx — Deep Links Visio Mobile

Ce guide explique comment configurer les instances Meet (nginx) pour que les liens
`https://` ouvrent directement l'application Visio Mobile sur Android, iOS et macOS.

## Prérequis

- Accès à la configuration nginx de chaque instance Meet
- Le SHA256 du certificat de signature Android
- Le Team ID Apple Developer

### Obtenir le SHA256 du certificat Android

```bash
# Depuis le keystore local
keytool -list -v -keystore <keystore.jks> -alias <alias> | grep SHA256

# Ou depuis Google Play Console :
# Configuration > Intégrité de l'application > Certificat de signature
```

### Obtenir le Team ID Apple

Visible sur https://developer.apple.com/account sous "Membership details".

## Configuration nginx

Ajouter le bloc suivant dans le `server {}` de **chaque instance Meet**
(ex: `meet.linagora.com`, `dev-meet.linagora.com`).

```nginx
# ==========================================================================
# Visio Mobile — Deep Links (App Links Android + Universal Links iOS/macOS)
# ==========================================================================

# Android App Links
# Documentation : https://developer.android.com/training/app-links/verify-android-applinks
location = /.well-known/assetlinks.json {
    default_type application/json;
    add_header Cache-Control "public, max-age=86400";
    return 200 '[{
        "relation": ["delegate_permission/common.handle_all_urls"],
        "target": {
            "namespace": "android_app",
            "package_name": "io.visio.mobile",
            "sha256_cert_fingerprints": [
                "XX:XX:XX:XX:XX:XX:XX:XX:XX:XX:XX:XX:XX:XX:XX:XX:XX:XX:XX:XX:XX:XX:XX:XX:XX:XX:XX:XX:XX:XX:XX:XX"
            ]
        }
    }]';
}

# iOS + macOS Universal Links
# Documentation : https://developer.apple.com/documentation/bundleresources/applinks
location = /.well-known/apple-app-site-association {
    default_type application/json;
    add_header Cache-Control "public, max-age=86400";
    return 200 '{
        "applinks": {
            "apps": [],
            "details": [
                {
                    "appID": "XXXXXXXXXX.io.visio.mobile",
                    "paths": ["/*"]
                }
            ]
        }
    }';
}
```

**Remplacer :**
- `XX:XX:...` par le SHA256 du certificat de signature Android (32 octets séparés par `:`)
- `XXXXXXXXXX` par le Team ID Apple Developer (10 caractères)

## Vérification

Après déploiement de la configuration et rechargement de nginx (`nginx -s reload`),
vérifier que les endpoints répondent correctement :

```bash
# Android App Links
curl -s https://meet.linagora.com/.well-known/assetlinks.json | jq .

# iOS/macOS Universal Links
curl -s https://meet.linagora.com/.well-known/apple-app-site-association | jq .
```

Les deux doivent :
- Retourner du JSON valide
- Etre servis en HTTPS (pas de redirection HTTP → HTTPS)
- Retourner un code 200

## Outils de validation

- **Android :** https://developers.google.com/digital-asset-links/tools/generator
  - Entrer le domaine et le package `io.visio.mobile` pour vérifier la configuration
- **iOS :** https://search.developer.apple.com/search?q=aasa-validator
  - Ou utiliser la commande : `curl -sI https://meet.linagora.com/.well-known/apple-app-site-association`

## Notes

- La configuration est **identique** pour chaque instance Meet (seul le `server_name` change)
- Les fichiers `.well-known/` ne nécessitent aucune modification du code Meet
- Le `Cache-Control` de 24h (86400s) est un bon compromis : les changements de certificat
  sont rares et Apple/Google re-vérifient régulièrement
- Si l'app Visio Mobile n'est pas installée, le lien ouvre Meet web normalement (fallback transparent)
