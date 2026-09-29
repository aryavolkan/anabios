//! Serde adapter for `Vec<[T; N]>` columns whose row width exceeds 32.
//!
//! serde 1.x derives `Serialize`/`Deserialize` for arrays only up to length
//! 32, and the meme vector (`program::MEME_CHANNELS`) outgrew that when the
//! projectile ladder appended Throwing Stones to the invention tree
//! (`FORMAT_VERSION` 45). Rows are written exactly the way the derived array
//! impl writes them — a fixed-length tuple with no per-row length prefix —
//! inside an ordinary length-prefixed sequence, so a column's bincode layout
//! is what the derive produced at width ≤ 32 (pinned by
//! `matches_the_derived_layout_below_32` below): `state_hash` semantics are
//! unchanged, only the row width grew. Mirrors the hand-rolled `Genome`
//! impls in `genome.rs`.

use std::marker::PhantomData;

use serde::de::{Deserializer, SeqAccess, Visitor};
use serde::ser::{SerializeSeq, SerializeTuple, Serializer};
use serde::{Deserialize, Serialize};

/// `#[serde(with = "crate::serde_rows::fixed_rows")]` for a `Vec<[T; N]>`
/// field of any width.
pub mod fixed_rows {
    use super::*;

    pub fn serialize<T: Serialize, const N: usize, S: Serializer>(
        rows: &[[T; N]],
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        let mut seq = serializer.serialize_seq(Some(rows.len()))?;
        for row in rows {
            seq.serialize_element(&Row(row))?;
        }
        seq.end()
    }

    pub fn deserialize<'de, T: Deserialize<'de>, const N: usize, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<Vec<[T; N]>, D::Error> {
        deserializer.deserialize_seq(RowsVisitor(PhantomData))
    }
}

/// One fixed-width row, serialized as a tuple — the derived array shape.
struct Row<'a, T, const N: usize>(&'a [T; N]);

impl<T: Serialize, const N: usize> Serialize for Row<'_, T, N> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut tup = serializer.serialize_tuple(N)?;
        for v in self.0.iter() {
            tup.serialize_element(v)?;
        }
        tup.end()
    }
}

/// One owned row, read back as a tuple of exactly `N` elements.
struct OwnedRow<T, const N: usize>([T; N]);

impl<'de, T: Deserialize<'de>, const N: usize> Deserialize<'de> for OwnedRow<T, N> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct RowVisitor<T, const N: usize>(PhantomData<T>);
        impl<'de, T: Deserialize<'de>, const N: usize> Visitor<'de> for RowVisitor<T, N> {
            type Value = OwnedRow<T, N>;

            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                write!(f, "a tuple of {N} values")
            }

            fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Self::Value, A::Error> {
                let mut items = Vec::with_capacity(N);
                for i in 0..N {
                    items.push(
                        seq.next_element()?
                            .ok_or_else(|| serde::de::Error::invalid_length(i, &self))?,
                    );
                }
                // Exactly `N` items were pushed, so the conversion cannot fail.
                let row: [T; N] =
                    items.try_into().unwrap_or_else(|_| unreachable!("row holds exactly N items"));
                Ok(OwnedRow(row))
            }
        }
        deserializer.deserialize_tuple(N, RowVisitor(PhantomData))
    }
}

/// The column: a length-prefixed sequence of rows.
struct RowsVisitor<T, const N: usize>(PhantomData<T>);

impl<'de, T: Deserialize<'de>, const N: usize> Visitor<'de> for RowsVisitor<T, N> {
    type Value = Vec<[T; N]>;

    fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "a sequence of {N}-wide rows")
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Self::Value, A::Error> {
        let mut out = Vec::with_capacity(seq.size_hint().unwrap_or(0));
        while let Some(OwnedRow(row)) = seq.next_element::<OwnedRow<T, N>>()? {
            out.push(row);
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use serde::{Deserialize, Serialize};

    /// A column wider than serde's derive limit (the meme vector's shape).
    #[derive(Debug, PartialEq, Serialize, Deserialize)]
    struct Wide {
        #[serde(with = "crate::serde_rows::fixed_rows")]
        rows: Vec<[f32; 33]>,
        #[serde(with = "crate::serde_rows::fixed_rows")]
        tags: Vec<[u32; 33]>,
    }

    #[test]
    fn round_trips_rows_wider_than_32() {
        let mut rows = Vec::new();
        let mut tags = Vec::new();
        for r in 0..5u32 {
            let mut row = [0.0f32; 33];
            let mut tag = [0u32; 33];
            for (c, (v, t)) in row.iter_mut().zip(tag.iter_mut()).enumerate() {
                *v = r as f32 + c as f32 * 0.25;
                *t = r * 100 + c as u32;
            }
            rows.push(row);
            tags.push(tag);
        }
        let wide = Wide { rows, tags };
        let bytes = bincode::serialize(&wide).expect("serialize");
        let back: Wide = bincode::deserialize(&bytes).expect("deserialize");
        assert_eq!(back, wide);
        // An empty column round-trips too (a world with no agents).
        let empty = Wide { rows: Vec::new(), tags: Vec::new() };
        let bytes = bincode::serialize(&empty).expect("serialize empty");
        let back: Wide = bincode::deserialize(&bytes).expect("deserialize empty");
        assert_eq!(back, empty);
    }

    #[derive(Serialize, Deserialize)]
    struct Adapted {
        #[serde(with = "crate::serde_rows::fixed_rows")]
        rows: Vec<[f32; 4]>,
    }

    #[derive(Serialize, Deserialize)]
    struct Derived {
        rows: Vec<[f32; 4]>,
    }

    /// Below the derive limit the adapter is byte-identical to the derive:
    /// widening a column never changes how its existing lanes are laid out.
    #[test]
    fn matches_the_derived_layout_below_32() {
        let rows = vec![[1.0f32, 2.0, 3.0, 4.0], [0.5, 0.25, 0.125, 0.0625]];
        let adapted = bincode::serialize(&Adapted { rows: rows.clone() }).expect("adapted");
        let derived = bincode::serialize(&Derived { rows }).expect("derived");
        assert_eq!(adapted, derived);
        let back: Derived = bincode::deserialize(&adapted).expect("derive reads adapter bytes");
        assert_eq!(back.rows.len(), 2);
        let back: Adapted = bincode::deserialize(&derived).expect("adapter reads derive bytes");
        assert_eq!(back.rows[1][2], 0.125);
    }

    /// A truncated row is a hard error, not a silently short array.
    #[test]
    fn rejects_a_truncated_row() {
        let derived = bincode::serialize(&Derived { rows: vec![[1.0f32, 2.0, 3.0, 4.0]] }).unwrap();
        let cut = &derived[..derived.len() - 4];
        assert!(bincode::deserialize::<Adapted>(cut).is_err());
    }
}
