use std::collections::HashMap;

use eframe::egui::{self, Context, TextureHandle};

use crate::core::{
    ArtworkDownloadResult, ArtworkManager, ArtworkRequestResult, GameEntry, PreparedArtwork,
};

const MAX_TEXTURE_UPLOADS_PER_TICK: usize = 6;

#[derive(Clone)]
pub(super) struct ArtworkTextures {
    pub(super) foreground: TextureHandle,
}

pub(super) struct ArtworkStore {
    manager: ArtworkManager,
    textures: HashMap<String, ArtworkTextures>,
}

impl ArtworkStore {
    pub(super) fn new() -> Self {
        Self {
            manager: ArtworkManager::new(),
            textures: HashMap::new(),
        }
    }

    pub(super) fn poll_download_results(&mut self, ctx: &Context) {
        let mut has_updates = false;

        for result in self
            .manager
            .poll_download_results(MAX_TEXTURE_UPLOADS_PER_TICK)
        {
            match result {
                ArtworkDownloadResult::Ready { key, payload } => {
                    if let Some(textures) = build_artwork_textures_from_payload(ctx, &key, payload)
                    {
                        self.textures.insert(key, textures);
                        has_updates = true;
                    }
                }
                ArtworkDownloadResult::Missing { .. } => {}
            }
        }

        if has_updates {
            ctx.request_repaint();
        }
    }

    pub(super) fn prepare_for_games(&mut self, games: &[GameEntry]) {
        let visible_keys = self.manager.prepare_for_games(games);
        self.textures.retain(|key, _| visible_keys.contains(key));
    }

    pub(super) fn refresh_metadata_for_games(&mut self, games: &[GameEntry]) {
        self.textures.clear();
        let visible_keys = self.manager.refresh_metadata_for_games(games);
        self.textures.retain(|key, _| visible_keys.contains(key));
    }

    pub(super) fn artwork_for_game(
        &mut self,
        ctx: &Context,
        game: &GameEntry,
    ) -> Option<ArtworkTextures> {
        let result = self.manager.request_for_game(game)?;
        self.resolve_artwork_request_result(ctx, result)
    }

    pub(super) fn mattmc_artwork(&mut self, ctx: &Context) -> Option<ArtworkTextures> {
        let result = self.manager.request_mattmc_artwork();
        self.resolve_artwork_request_result(ctx, result)
    }

    fn resolve_artwork_request_result(
        &mut self,
        ctx: &Context,
        result: ArtworkRequestResult,
    ) -> Option<ArtworkTextures> {
        let key = match &result {
            ArtworkRequestResult::Ready { key, .. }
            | ArtworkRequestResult::Pending { key }
            | ArtworkRequestResult::Missing { key } => key,
        };

        if let Some(existing_texture) = self.textures.get(key) {
            return Some(existing_texture.clone());
        }

        match result {
            ArtworkRequestResult::Ready { key, payload } => {
                let textures = build_artwork_textures_from_payload(ctx, &key, payload)?;
                self.textures.insert(key, textures.clone());
                Some(textures)
            }
            ArtworkRequestResult::Pending { .. } | ArtworkRequestResult::Missing { .. } => None,
        }
    }
}

fn build_artwork_textures_from_payload(
    ctx: &Context,
    key: &str,
    payload: PreparedArtwork,
) -> Option<ArtworkTextures> {
    let foreground_color_image =
        egui::ColorImage::from_rgba_unmultiplied([payload.width, payload.height], &payload.rgba);

    let foreground = ctx.load_texture(
        format!("game-artwork-fg-{}", key),
        foreground_color_image,
        egui::TextureOptions::LINEAR,
    );

    Some(ArtworkTextures { foreground })
}
