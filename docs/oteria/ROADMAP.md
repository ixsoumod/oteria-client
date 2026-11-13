# Oteria Client — feuille de route

Ce document liste les ajouts prévus pour Oteria Client, classés **du plus dur au plus simple**, avec ce qui
existe déjà dans le moteur (hérité d'Horizon), ce qu'il faut construire et un ordre de réalisation conseillé.
Les estimations supposent une personne qui développe avec Claude ; elles servent à comparer, pas à promettre.

Légende de difficulté : 🔴 très dur · 🟠 dur · 🟡 moyen · 🟢 facile.

## Ce qui est déjà fait

- Fork d'Horizon renommé Oteria, dépôt public `ixsoumod/oteria-client`, aucun service d'Horizon contacté.
- Dossiers de données propres (`%APPDATA%\oteria`, `oteria-launcher`), lanceur `oteria.cmd`, compilations
  rapides (`oteria-build.cmd`, ~2 min).
- Identité visuelle : logo, icônes, thème bleu, badge Oteria devant ton pseudo (Tab et au-dessus de la tête).
- Menus de réglages en verre, préréglages en un clic, FPS dans le menu Vidéo.
- Chaîne de publication signée (`tools/oteria/publish.ps1`), en attente de la première version.
- Connexion Microsoft : application Azure créée, en attente de la validation de Mojang.

## Classement, du plus dur au plus simple

### 1. 🔴 Rendre les mods Java compatibles

**La réalité technique.** Un mod Fabric/Forge est du bytecode Java qui modifie l'intérieur de Minecraft
(Mixins). Oteria n'a ni JVM ni le code de Minecraft : il n'y a rien à quoi ces mods peuvent s'accrocher.
Un **convertisseur automatique** du code Java vers Oteria n'est **pas réaliste** : il faudrait traduire du
code arbitraire qui suppose l'architecture interne de Minecraft Java.

**Ce qui est possible.**
- *Déjà là* : le launcher reprend les **données et les assets** des mods Java et des modpacks (`.mrpack`,
  CurseForge, Prism) : blocs, objets, recettes, textures (`crates/horizon-port-pack`).
- **Recréer les mods populaires en Rust**, un par un, sous forme de mods natifs (`.hnmod`) ou directement
  intégrés : c'est ce qui a été fait pour Zoom, la minimap, les lumières dynamiques… Les mods de performance
  (Sodium, Lithium, FerriteCore) sont **inutiles** : le moteur Rust fait déjà mieux.
- Priorité suggérée : les mods PvP et confort (voir l'étape 9), puis les mods de contenu les plus demandés.

### 2. 🔴 Changer les mods à chaud (comme PlugMan)

**La réalité technique.** Les mods natifs d'Oteria sont du Rust **compilé dans le jeu** par le launcher. Rust
n'a pas d'interface binaire stable : charger et décharger du code compilé pendant que le jeu tourne est
fragile (plantages si l'interface change).

**Ce qui est possible.**
- 🟡 **Mods de contenu à chaud** (`.hmod` : blocs, objets, recettes, textures, sans code) : recharger ces
  fichiers pendant la partie est réaliste, comme un `/reload` de datapack.
- 🟠 **Une couche de scripts pour les mods de code** (WASM ou Lua, en bac à sable) : ces scripts se chargent
  et se déchargent à chaud sans risque pour le jeu. Horizon a exploré les deux pistes (branches
  `feat/mods-lua` et l'ancien support WASM), ce qui donne une base.
- 🔴 Recharger à chaud des mods natifs compilés : à éviter.

### 3. 🟠 Simple Voice Chat intégré

**La réalité technique.** Simple Voice Chat fonctionne avec un plugin côté serveur et son propre protocole
(UDP, audio Opus, chiffrement). Pour parler sur les serveurs qui ont ce plugin, Oteria doit **réimplémenter
le client** de ce protocole en Rust : capture micro, encodage Opus, réseau, son spatialisé, réglages
(push-to-talk, volume par joueur, groupes).

**Faisable**, mais c'est un gros morceau : à attaquer une fois le reste solide. Bonus possible : la voix en
solo/LAN entre joueurs Oteria, sans plugin.

### 4. 🟠 Des shaders Oteria, beaux et légers

**Déjà là** : le moteur lit les shader packs Iris/OptiFine **et** ses propres packs en WGSL (`.hshader`,
`docs/shaders.md`), avec HDR, ombres, transparence améliorée et un rendu en Vulkan/DX12.

**À construire** : un pack « Oteria Shaders » pensé pour la performance, en plusieurs niveaux :
1. *Léger* : ciel et brouillard plus beaux, eau animée, couleurs (tonemapping) — presque gratuit.
2. *Moyen* : ombres douces à distance limitée, bloom, reflets d'eau simples.
3. *Élevé* : ombres plus loin, lumière volumétrique, reflets plus fins.
Demande des compétences en rendu, mais se fait par petites étapes, chacune visible tout de suite.

### 5. 🟠 Cosmétiques

**Déjà là** : tout le système côté jeu (capes, chapeaux, ailes, emotes, styles de pseudo, garde-robe dans le
launcher, `crates/horizon-cosmetics`). Il manque :
- un **service en ligne Oteria** (serveur à héberger) qui dit qui porte quoi — c'est aussi lui qui permettra
  d'afficher le **badge Oteria devant les pseudos des autres joueurs Oteria** ;
- les **modèles et textures** des cosmétiques (travail d'artiste) ;
- éventuellement une boutique — à faire seulement en respectant les règles commerciales de Minecraft (pas
  d'avantage en jeu, uniquement du cosmétique).

### 6. 🟡 Mode « FPS Boost »

**Promesse honnête** : un **×5 garanti n'est pas réaliste** — Oteria est déjà plus rapide que Minecraft Java
(×1,2 à ×1,4 de FPS d'après les mesures d'Horizon, chargements ×3), et le gain dépend du PC. En revanche, un
mode qui **tire le maximum de chaque PC** est tout à fait faisable :
- détecter la machine (processeur, carte graphique, mémoire : `crates/horizon-system`) ;
- lancer un **mini benchmark** de 10 secondes et ajuster automatiquement distance d'affichage, distance LOD,
  culling, particules, ombres ;
- activer l'**upscaling** et la **génération d'images** déjà présents dans le moteur quand la carte le permet
  (c'est là qu'on peut doubler ou tripler les FPS affichés) ;
- une **cible de FPS** (ex. 240) que le jeu tient en adaptant la qualité en direct.
**Chargement des mondes** : déjà ×3 plus rapide ; on peut aller plus loin avec la prégénération (`/pregen`
existe) et un cache des chunks proches du spawn.

### 7. 🟡 Rejoindre un ami / héberger son monde en un clic

**Déjà là** : « Play with Friends » d'Horizon (codes d'invitation, tunnel chiffré, découverte NAT,
`crates/horizon-friends`, `crates/horizon-relay`), coupé chez nous parce qu'il passait par le relais
d'Horizon. Il faut :
- héberger **le relais Oteria** (petit serveur, le code existe) ;
- un bouton « Héberger » et « Rejoindre avec un code » dans le menu, et une liste d'amis.

### 8. 🟡 Le launcher : mods, modpacks, resource packs, shader packs

**Déjà là**, dans le launcher d'Horizon : instances, page Mods (registre + Modrinth), modpacks, resource
packs, shader packs, data packs, serveurs, captures d'écran, mises à jour automatiques. Il reste :
- finir l'**étape 6** (première version publiée, installateur pour tes potes) ;
- le **restyler** au look Oteria (verre, bleu) comme les menus du jeu ;
- brancher un **registre de mods Oteria** au lieu de celui d'Horizon.

### 9. 🟢 Menu Shift droit avec les mods PvP intégrés

**Déjà là**, dispersé dans les menus : compteur de FPS, keystrokes (avec CPS), coordonnées, zoom, minimap,
fullbright, lumières dynamiques, Mouse Tweaks, reconnexion auto (`extras`, `pvp_hud`).
**À construire** : un **menu en verre ouvert avec Shift droit**, en grille de tuiles (une par mod :
activer/désactiver, réglages, déplacement sur l'écran), qui remplace le bouton « Oteria Settings » retiré.
Mods PvP à ajouter ensuite : **Armor HUD**, **Potion HUD**, **CPS seul**, **ping**, **réticule
personnalisé**, **Toggle Sprint/Sneak**, **Freelook**, **couleur des coups**, **compteur de combo**,
**horloge**, **direction (boussole)**, **Item Physics**, **feu bas / bouclier bas**.

### 10. 🟢 Discord Rich Presence

Afficher sur ton profil Discord que tu joues à Oteria : le logo, le serveur ou le monde (« Joue sur Craftok »),
le mode (PvP, survie), le temps de jeu, et un bouton « Rejoindre » quand le serveur l'autorise.
**Déjà là** : l'intégration Discord d'Horizon (`crates/horizon-discord`, « Playing Horizon »), retirée de notre
build avec ses autres services. Il faut créer **l'application Discord Oteria** (son identifiant et le logo sur
le portail développeur de Discord), puis rebrancher le module avec ces informations, et une option pour la
couper ou masquer le serveur.

### 11. 🟢 Profils

Des jeux de réglages qu'on change en un clic : « PvP » (FPS max, HUD PvP), « Survie », « Build »,
« Enregistrement ». Possibilité de lier un profil à un serveur (ex. Craftok → profil PvP automatiquement).

## Ordre conseillé

| Phase | Contenu | Pourquoi d'abord |
| --- | --- | --- |
| 1 | Menu Shift droit + mods PvP (9), Discord Rich Presence (10), profils (11) | Rapide, très visible, c'est le cœur d'un client PvP |
| 2 | Publication + launcher restylé (8), FPS Boost (6) | Permet à tes potes d'installer et de sentir la différence |
| 3 | Rejoindre / héberger en 1 clic (7), service en ligne Oteria | Le serveur sert aussi aux cosmétiques et aux badges |
| 4 | Cosmétiques (5), shaders Oteria niveau léger puis moyen (4) | Plus de travail d'art et de rendu |
| 5 | Voice chat (3), mods à chaud (contenu puis scripts) (2), mods Java recréés (1) | Les plus gros chantiers |

## File d'attente

- **Tier Tagger** (comme le mod Fabric `tiertagger`) : le tier PvP de chaque joueur (MCTiers et autres listes) à
  côté de son pseudo, au-dessus de sa tête et dans la liste des joueurs, avec le mode de jeu choisi. Demandé le
  12 novembre 2026, à faire après le chat vocal et Mouse Tweaks.

## Idées en plus

- **Intégration OteriaPvP Practice** : bouton « Jouer à Oteria Practice » sur l'écran titre, statistiques de
  ton profil (ELO, kills) dans le menu Shift droit, découpe des kits.
- **Replay** : le moteur a déjà un « Replay Studio » — enregistrer ses combats et les revoir sous tous les
  angles, export en vidéo.
- **Hit-delay / CPS graph** et **analyse de combat** après un match (dégâts, combos, précision).
- **Chat amélioré** : horodatage, recherche, copier un message, empilement des messages répétés.
- **Waypoints** et **minimap de mort** (point de mort affiché).
- **Overlay de performance** avancé (temps CPU/GPU, graphe de frame time) pour vérifier le FPS Boost.
- **Capture d'écran en un clic** avec upload et lien copié.
- **Packs par serveur** : charger automatiquement un resource pack quand tu rejoins un serveur donné.

## Règles à garder en tête

- **Équité** : aucun mod ne doit donner d'avantage interdit (reach, autoclicker, X-ray, kill aura). Beaucoup
  de serveurs PvP ont des listes de mods autorisés : rester dans ce cadre évite que les joueurs Oteria soient
  bannis.
- **Licence** : le code d'Horizon est sous MIT, les mentions de copyright restent dans `LICENSE`.
- **Mojang** : aucun fichier de Minecraft dans le dépôt ni dans les versions publiées ; les cosmétiques et
  boutiques suivent les règles commerciales de Minecraft.
