//! Asset source: GPUI Kit's default component icons plus the extra Lucide
//! icons this app uses. Only the listed SVGs are embedded.

use gpui_kit::*;
use std::borrow::Cow;

gpui_kit::assets::icon_assets!(
    pub ExtraIcons,
    [
        Send,
        Server,
        Timer,
        Code,
        Trash,
        Plug,
        Unplug,
        Pencil,
        Save,
        Download,
        Upload,
        Radio,
        MessageSquare,
        Braces,
        Clock,
        Zap,
        Power,
        Layers,
        SquarePen,
        FileCode,
        ListChecks,
        CircleStop,
        Eraser,
        Terminal,
        Binary,
    ]
);

pub struct AppAssets;

impl AssetSource for AppAssets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        if let Some(bytes) = ExtraIcons.load(path)? {
            return Ok(Some(bytes));
        }
        gpui_kit::assets::Assets.load(path)
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        let mut paths = gpui_kit::assets::Assets.list(path)?;
        paths.extend(ExtraIcons.list(path)?);
        paths.sort();
        paths.dedup();
        Ok(paths)
    }
}
