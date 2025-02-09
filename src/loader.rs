use std::path::Path;

use futures::FutureExt;

use crate::{
    parse::xml::{AsyncReadFrom, SyncReadFrom},
    AsyncResourceReader, DefaultResourceCache, FilesystemResourceReader, Map, ResourceCache,
    ResourceReader, Result, Tileset,
};

/// A type used for loading [`Map`]s and [`Tileset`]s.
///
/// Internally, it holds a [`ResourceCache`] that, as its name implies, caches intermediate loading
/// artifacts, most notably map tilesets.
///
/// It also contains a [`ResourceReader`] which is the object in charge of providing read handles
/// to files via a [`ResourcePath`](crate::ResourcePath).
///
/// ## Reasoning
/// This type is used for loading operations because they require a [`ResourceCache`] for
/// intermediate artifacts, so using a type for creation can ensure that the cache is reused if
/// loading more than one object is required.
#[derive(Debug, Clone, Default)]
pub struct Loader<Reader = FilesystemResourceReader, Cache: ResourceCache = DefaultResourceCache> {
    cache: Cache,
    reader: Reader,
}

impl Loader {
    /// Creates a new loader, creating a default resource cache and reader
    /// ([`DefaultResourceCache`] & [`FilesystemResourceReader`] respectively) in the process.
    pub fn new() -> Self {
        Self {
            cache: DefaultResourceCache::new(),
            reader: FilesystemResourceReader::new(),
        }
    }
}

impl<Reader> Loader<Reader, DefaultResourceCache> {
    /// Creates a new loader using a specific reader and the default resource cache ([`DefaultResourceCache`]).
    /// Shorthand for `Loader::with_cache_and_reader(DefaultResourceCache::new(), reader)`.
    ///
    /// ## Example
    /// ```
    /// # fn main() -> tiled::Result<()> {
    /// use std::{sync::Arc, path::Path};
    ///
    /// use tiled::{Loader, ResourceCache};
    ///
    /// let mut loader = Loader::with_reader(
    ///     // Specify the reader to use. We can use anything that implements `ResourceReader`, e.g. FilesystemResourceReader.
    ///     // Any function that has the same signature as `ResourceReader::read_from` also implements it.
    ///     // Here we define a reader that embeds the map at "assets/tiled_xml.csv" into the executable, and allow
    ///     // accessing it only through "/my-map.tmx"
    ///     // ALL maps, tilesets and templates will be read through this function, even if you don't explicitly load them
    ///     // (They can be dependencies of one you did want to load in the first place).
    ///     // Doing this embedding is useful for places where the OS filesystem is not available (e.g. WASM applications).
    ///     |path: &std::path::Path| -> std::io::Result<_> {
    ///         if path == std::path::Path::new("/my-map.tmx") {
    ///             Ok(std::io::Cursor::new(include_bytes!("../assets/tiled_csv.tmx")))
    ///         } else {
    ///             Err(std::io::ErrorKind::NotFound.into())
    ///         }
    ///     }
    /// );
    ///
    /// let map = loader.load_tmx_map("/my-map.tmx")?;
    ///
    /// assert_eq!(
    ///     map.tilesets()[0].image.as_ref().unwrap().source,
    ///     Path::new("/tilesheet.png")
    /// );
    ///
    /// # Ok(())
    /// # }
    /// ```
    pub fn with_reader(reader: Reader) -> Self {
        Self {
            cache: DefaultResourceCache::new(),
            reader,
        }
    }
}

impl<Reader, Cache: ResourceCache> Loader<Reader, Cache> {
    /// Creates a new loader using a specific resource cache and reader. In most cases you won't
    /// need a custom resource cache; If that is the case you can use [`Loader::with_reader()`] for
    /// a less verbose version of this function.
    ///
    /// ## Example
    /// ```
    /// # fn main() -> tiled::Result<()> {
    /// use std::{sync::Arc, path::Path};
    ///
    /// use tiled::{Loader, ResourceCache};
    ///
    /// /// An example resource cache that doesn't actually cache any resources at all.
    /// struct NoopResourceCache;
    ///
    /// impl ResourceCache for NoopResourceCache {
    ///     fn get_tileset(
    ///         &self,
    ///         _path: impl AsRef<tiled::ResourcePath>,
    ///     ) -> Option<std::sync::Arc<tiled::Tileset>> {
    ///         None
    ///     }
    ///
    ///     fn get_template(
    ///         &self,
    ///         _path: impl AsRef<tiled::ResourcePath>,
    ///     ) -> Option<std::sync::Arc<tiled::Template>> {
    ///         None
    ///     }
    ///
    ///     fn insert_tileset(
    ///         &mut self,
    ///         _path: impl AsRef<tiled::ResourcePath>,
    ///         _tileset: Arc<tiled::Tileset>
    ///     ) {}
    ///
    ///     fn insert_template(
    ///         &mut self,
    ///         _path: impl AsRef<tiled::ResourcePath>,
    ///         _template: Arc<tiled::Template>
    ///     ) {}
    /// }
    ///
    /// let mut loader = Loader::with_cache_and_reader(
    ///     // Specify the resource cache to use. In this case, the one we defined earlier.
    ///     NoopResourceCache,
    ///     // Specify the reader to use. We can use anything that implements `ResourceReader`, e.g. FilesystemResourceReader.
    ///     // Any function that has the same signature as `ResourceReader::read_from` also implements it.
    ///     // Here we define a reader that embeds the map at "assets/tiled_xml.csv" into the executable, and allow
    ///     // accessing it only through "/my-map.tmx"
    ///     // ALL maps, tilesets and templates will be read through this function, even if you don't explicitly load them
    ///     // (They can be dependencies of one you did want to load in the first place).
    ///     // Doing this embedding is useful for places where the OS filesystem is not available (e.g. WASM applications).
    ///     |path: &std::path::Path| -> std::io::Result<_> {
    ///         if path == std::path::Path::new("/my-map.tmx") {
    ///             Ok(std::io::Cursor::new(include_bytes!("../assets/tiled_csv.tmx")))
    ///         } else {
    ///             Err(std::io::ErrorKind::NotFound.into())
    ///         }
    ///     }
    /// );
    ///
    /// let map = loader.load_tmx_map("/my-map.tmx")?;
    ///
    /// assert_eq!(
    ///     map.tilesets()[0].image.as_ref().unwrap().source,
    ///     Path::new("/tilesheet.png")
    /// );
    ///
    /// # Ok(())
    /// # }
    /// ```
    pub fn with_cache_and_reader(cache: Cache, reader: Reader) -> Self {
        Self { cache, reader }
    }

    /// Returns a reference to the loader's internal [`ResourceCache`].
    pub fn cache(&self) -> &Cache {
        &self.cache
    }

    /// Returns a mutable reference to the loader's internal [`ResourceCache`].
    pub fn cache_mut(&mut self) -> &mut Cache {
        &mut self.cache
    }

    /// Returns a reference to the loader's internal [`ResourceReader`].
    pub fn reader(&self) -> &Reader {
        &self.reader
    }

    /// Returns a mutable reference to the loader's internal [`ResourceReader`].
    pub fn reader_mut(&mut self) -> &mut Reader {
        &mut self.reader
    }

    /// Consumes the loader and returns its internal [`ResourceCache`] and [`ResourceReader`].
    pub fn into_inner(self) -> (Cache, Reader) {
        (self.cache, self.reader)
    }
}

impl<Reader: ResourceReader, Cache: ResourceCache> Loader<Reader, Cache> {
    /// Parses a file hopefully containing a Tiled map and tries to parse it. All external files
    /// will be loaded relative to the path given.
    ///
    /// All intermediate objects such as map tilesets will be stored in the [internal loader cache].
    ///
    /// [internal loader cache]: Loader::cache()
    pub fn load_tmx_map(&mut self, path: impl AsRef<Path>) -> Result<Map> {
        let mut read_from = SyncReadFrom(&mut self.reader);
        crate::parse::xml::parse_map(path.as_ref(), &mut read_from, &mut self.cache)
            .now_or_never()
            .expect(
                "synchronously loading a TMX map stayed pending; this is a bug, please report it",
            )
    }

    /// Parses a file hopefully containing a Tiled tileset and tries to parse it. All external files
    /// will be loaded relative to the path given.
    ///
    /// Unless you specifically want to load a tileset, you won't need to call this function. If
    /// you are trying to load a map, simply use [`Loader::load_tmx_map`].
    ///
    /// ## Note
    /// This function will **not** cache the tileset inside the internal [`ResourceCache`], since
    /// in this context it is not an intermediate object.
    pub fn load_tsx_tileset(&mut self, path: impl AsRef<Path>) -> Result<Tileset> {
        let mut read_from = SyncReadFrom(&mut self.reader);
        crate::parse::xml::parse_tileset(path.as_ref(), &mut read_from, &mut self.cache)
            .now_or_never()
            .expect(
                "synchronously loading a TSX tileset stayed pending; this is a bug, please report it",
            )
    }
}

impl<Reader: AsyncResourceReader, Cache: ResourceCache> Loader<Reader, Cache> {
    /// Parses a file hopefully containing a Tiled map and tries to parse it. All external files
    /// will be loaded relative to the path given.
    ///
    /// All intermediate objects such as map tilesets will be stored in the [internal loader cache].
    ///
    /// [internal loader cache]: Loader::cache()
    pub async fn load_tmx_map_async(&mut self, path: impl AsRef<Path>) -> Result<Map> {
        let mut read_from = AsyncReadFrom(&mut self.reader);
        crate::parse::xml::parse_map(path.as_ref(), &mut read_from, &mut self.cache).await
    }

    /// Parses a file hopefully containing a Tiled tileset and tries to parse it. All external files
    /// will be loaded relative to the path given.
    ///
    /// Unless you specifically want to load a tileset, you won't need to call this function. If
    /// you are trying to load a map, simply use [`Loader::load_tmx_map`].
    ///
    /// ## Note
    /// This function will **not** cache the tileset inside the internal [`ResourceCache`], since
    /// in this context it is not an intermediate object.
    pub async fn load_tsx_tileset_async(&mut self, path: impl AsRef<Path>) -> Result<Tileset> {
        let mut read_from = AsyncReadFrom(&mut self.reader);
        crate::parse::xml::parse_tileset(path.as_ref(), &mut read_from, &mut self.cache).await
    }
}
