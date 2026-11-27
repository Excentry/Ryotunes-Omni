#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MediaRef<'a> {
    YouTube(&'a str),
    Local(&'a str),
    Radio(&'a str),
    SpotifyTrack(&'a str),
    SpotifyArtist(&'a str),
    SpotifyAlbum(&'a str),
    SpotifyPlaylist(&'a str),
    SpotifyOther(&'a str),
    SoundCloudTrack(u64),
    SoundCloudUser(u64),
    SoundCloudPlaylist(u64),
    SoundCloudSystem(&'a str),
    SoundCloudOther(&'a str),
    TidalTrack(u64),
    TidalArtist(u64),
    TidalAlbum(u64),
    TidalPlaylist(&'a str),
    TidalOther(&'a str),
}

impl<'a> MediaRef<'a> {
    pub fn parse(id: &'a str) -> Self {
        if let Some(path) = crate::local::song_path(id) {
            return Self::Local(path);
        }
        if crate::radio::is_radio_id(id) {
            return Self::Radio(id);
        }
        if let Some(track_id) = crate::spotify::spotify_track_id(id) {
            return Self::SpotifyTrack(track_id);
        }
        if let Some(artist_id) = id.strip_prefix("spotify:artist:") {
            return Self::SpotifyArtist(artist_id);
        }
        if let Some(album_id) = id.strip_prefix("spotify:album:") {
            return Self::SpotifyAlbum(album_id);
        }
        if let Some(playlist_id) = id.strip_prefix("spotify:playlist:") {
            return Self::SpotifyPlaylist(playlist_id);
        }
        if crate::spotify::is_spotify_id(id) {
            return Self::SpotifyOther(id);
        }
        if let Some(track_id) = crate::spotify::sc_track_id(id) {
            return Self::SoundCloudTrack(track_id);
        }
        if let Some(user_id) = crate::spotify::sc_user_id(id) {
            return Self::SoundCloudUser(user_id);
        }
        if let Some(playlist_id) = crate::spotify::sc_playlist_id(id) {
            return Self::SoundCloudPlaylist(playlist_id);
        }
        if let Some(permalink) = crate::spotify::sc_system_id(id) {
            return Self::SoundCloudSystem(permalink);
        }
        if crate::spotify::is_sc_id(id) {
            return Self::SoundCloudOther(id);
        }
        if let Some(track_id) = crate::spotify::tidal_track_id(id) {
            return Self::TidalTrack(track_id);
        }
        if let Some(artist_id) = crate::spotify::tidal_artist_id(id) {
            return Self::TidalArtist(artist_id);
        }
        if let Some(album_id) = crate::spotify::tidal_album_id(id) {
            return Self::TidalAlbum(album_id);
        }
        if let Some(playlist_id) = crate::spotify::tidal_playlist_id(id) {
            return Self::TidalPlaylist(playlist_id);
        }
        if crate::spotify::is_tidal_id(id) {
            return Self::TidalOther(id);
        }
        Self::YouTube(id)
    }
}

#[cfg(test)]
mod tests {
    use super::MediaRef;

    #[test]
    fn parses_each_supported_playback_namespace() {
        assert_eq!(MediaRef::parse("abcdefghijk"), MediaRef::YouTube("abcdefghijk"));
        assert_eq!(MediaRef::parse("LOCAL:/music/a.flac"), MediaRef::Local("/music/a.flac"));
        assert_eq!(
            MediaRef::parse("RYOTUNES_RADIO:station"),
            MediaRef::Radio("RYOTUNES_RADIO:station")
        );
        assert_eq!(MediaRef::parse("spotify:track:abc123"), MediaRef::SpotifyTrack("abc123"));
        assert_eq!(MediaRef::parse("spotify:album:abc123"), MediaRef::SpotifyAlbum("abc123"));
        assert_eq!(MediaRef::parse("spotify:artist:abc123"), MediaRef::SpotifyArtist("abc123"));
        assert_eq!(MediaRef::parse("spotify:playlist:abc123"), MediaRef::SpotifyPlaylist("abc123"));
        assert_eq!(MediaRef::parse("sc:track:42"), MediaRef::SoundCloudTrack(42));
        assert_eq!(MediaRef::parse("sc:user:42"), MediaRef::SoundCloudUser(42));
        assert_eq!(MediaRef::parse("sc:playlist:42"), MediaRef::SoundCloudPlaylist(42));
        assert_eq!(MediaRef::parse("sc:system:charts"), MediaRef::SoundCloudSystem("charts"));
        assert_eq!(MediaRef::parse("tidal:track:314"), MediaRef::TidalTrack(314));
        assert_eq!(MediaRef::parse("tidal:artist:314"), MediaRef::TidalArtist(314));
        assert_eq!(MediaRef::parse("tidal:album:314"), MediaRef::TidalAlbum(314));
        assert_eq!(MediaRef::parse("tidal:playlist:uuid"), MediaRef::TidalPlaylist("uuid"));
    }

    #[test]
    fn keeps_unhandled_provider_ids_out_of_the_youtube_variant() {
        assert_eq!(
            MediaRef::parse("sc:track:not-a-number"),
            MediaRef::SoundCloudOther("sc:track:not-a-number")
        );
        assert_eq!(MediaRef::parse("tidal:artist:bad"), MediaRef::TidalOther("tidal:artist:bad"));
        assert_eq!(
            MediaRef::parse("spotify:future-kind:item"),
            MediaRef::SpotifyOther("spotify:future-kind:item")
        );
    }
}
