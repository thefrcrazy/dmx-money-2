const normalizeAssetPath = (path: string) => path.replace(/^\/+/, '');

export const publicAsset = (path: string) => `${import.meta.env.BASE_URL}${normalizeAssetPath(path)}`;

export const LOGO_PATH = publicAsset('logo.png');
