# Icônes du thème Adwaita

Icônes symboliques du thème [Adwaita](https://gitlab.gnome.org/GNOME/adwaita-icon-theme) utilisées
par l'app Linux. Elles sont embarquées sous un nom préfixé (`dmx-adwaita-<nom>-symbolic`), pour
s'afficher même là où le thème Adwaita n'est pas installé : autre bureau que GNOME, AppImage.

Le thème Adwaita est distribué, au choix, sous licence
[GNU LGPL v3](https://www.gnu.org/licenses/lgpl-3.0.html) ou
[Creative Commons Attribution-Share Alike 3.0 United States](https://creativecommons.org/licenses/by-sa/3.0/us/),
© les auteurs du projet GNOME. Ces fichiers ne sont pas modifiés et restent sous cette licence.

`scripts/gen-native-icons.py --fetch --adwaita <liste>` télécharge celles que
`shared/icons/native.json` référence (`adwaita:<nom>`) et les copie dans l'app Linux.
