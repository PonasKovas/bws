use crate::world::{WorldChunk, WorldChunks};
use anyhow::{bail, Result};
use bytecheck::CheckBytes;
use rkyv::{
    check_archived_root,
    ser::{serializers::AllocSerializer, Serializer},
    with::{ArchiveWith, DeserializeWith, SerializeWith},
    Archive, ArchiveUnsized, Deserialize, DeserializeUnsized, Fallible, Infallible, Serialize,
    SerializeUnsized,
};
use std::collections::HashMap;
use std::marker::PhantomData;
use std::{borrow::Cow, io::Write, rc::Rc};
use tokio::{
    fs::File,
    io::{AsyncReadExt, AsyncWriteExt},
};

pub const VERSION: u32 = 1;

pub struct AsOwned;

impl<'a, T: 'a + ToOwned + ?Sized + ArchiveUnsized> ArchiveWith<Cow<'a, T>> for AsOwned {
    type Archived = rkyv::boxed::ArchivedBox<<T as ArchiveUnsized>::Archived>;
    type Resolver = rkyv::boxed::BoxResolver<<T as ArchiveUnsized>::MetadataResolver>;

    unsafe fn resolve_with(
        field: &Cow<'a, T>,
        pos: usize,
        resolver: Self::Resolver,
        out: *mut Self::Archived,
    ) {
        rkyv::boxed::ArchivedBox::resolve_from_ref(field.as_ref(), pos, resolver, out);
    }
}
impl<'a, T: 'a + ToOwned + ?Sized, S: Fallible + ?Sized> SerializeWith<Cow<'a, T>, S> for AsOwned
where
    T: SerializeUnsized<S>,
{
    fn serialize_with(field: &Cow<'a, T>, serializer: &mut S) -> Result<Self::Resolver, S::Error> {
        rkyv::boxed::ArchivedBox::serialize_from_ref(field.as_ref(), serializer)
    }
}
impl<'a, T, D> DeserializeWith<rkyv::boxed::ArchivedBox<T::Archived>, Cow<'a, T>, D> for AsOwned
where
    T: 'a + ToOwned + ArchiveUnsized + ?Sized,
    T::Archived: DeserializeUnsized<T, D>,
    D: Fallible + ?Sized,
{
    fn deserialize_with(
        field: &rkyv::boxed::ArchivedBox<T::Archived>,
        deserializer: &mut D,
    ) -> Result<Cow<'a, T>, D::Error> {
        Ok(Cow::Owned(
            Deserialize::<Box<T>, D>::deserialize(field, deserializer)?.to_owned(),
        ))
    }
}

#[derive(Archive, Deserialize, Serialize, Debug)]
#[archive_attr(derive(CheckBytes))]
pub struct Map<'a, const CHUNKS: usize> {
    #[with(AsOwned)]
    pub chunks: Cow<'a, [Box<WorldChunk>; CHUNKS]>,
    pub extra: HashMap<String, Vec<u8>>,
}

impl<'a, const CHUNKS: usize> Map<'a, CHUNKS> {
    pub async fn load(path: &str) -> Result<Map<'a, CHUNKS>> {
        let mut file = File::open(path).await?;

        let mut version = [0u8; std::mem::size_of::<u32>()];
        file.read_exact(&mut version).await?;
        let version = u32::from_be_bytes(version);
        if version != VERSION {
            bail!("Format version not compatible");
        }

        let mut chunks = [0u8; std::mem::size_of::<u32>()];
        file.read_exact(&mut chunks).await?;
        let chunks = u32::from_be_bytes(chunks);
        if chunks as usize != CHUNKS {
            bail!("Not compatible map size. The given map is of {} chunks, while trying to read a map of {} chunks", chunks, CHUNKS);
        }

        let mut buf = Vec::new();
        file.read_to_end(&mut buf).await?;

        let archived = check_archived_root::<Map<'a, CHUNKS>>(buf.as_slice()).unwrap();

        Ok(archived.deserialize(&mut Infallible).unwrap())
    }
    pub async fn save(&self, path: &str) -> Result<()> {
        let mut file = File::create(path).await?;

        let buf = {
            let mut serializer = AllocSerializer::<4096>::default();
            serializer.serialize_value(self).unwrap();

            serializer.into_serializer().into_inner()
        };

        file.write_all(&VERSION.to_be_bytes()[..]).await?;
        file.write_all(&(CHUNKS as u32).to_be_bytes()[..]).await?;

        file.write_all(&buf).await?;

        Ok(())
    }
}
