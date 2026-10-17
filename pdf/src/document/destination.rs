//! Explicit, document-local destinations. Null coordinates retain reader state.

use super::{PdfDocument, identity::ObjectHandle};
use crate::{
    error::{PdfError, PdfResult},
    objects::{
        PdfObject, pdf_array::PdfArray, pdf_name::PdfName, pdf_number::PdfNumber,
        pdf_reference::PdfReference,
    },
};
use std::io::{Read, Seek};

#[derive(Debug, Clone, PartialEq)]
pub enum DestinationView {
    Xyz {
        left: Option<f32>,
        top: Option<f32>,
        zoom: Option<f32>,
    },
    Fit,
    FitH(Option<f32>),
    FitV(Option<f32>),
    FitR {
        left: f32,
        bottom: f32,
        right: f32,
        top: f32,
    },
    FitB,
    FitBH(Option<f32>),
    FitBV(Option<f32>),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Destination {
    pub page: ObjectHandle,
    pub view: DestinationView,
}

impl<R: Read + Seek> PdfDocument<R> {
    /// Capture a page address that remains bound to that page across reordering.
    pub fn destination(&self, page_index: u32, view: DestinationView) -> PdfResult<Destination> {
        let dest = Destination {
            page: self.object_handle(self.page_location(page_index)?.page_id)?,
            view,
        };
        self.encode_destination(&dest)?;
        Ok(dest)
    }

    pub fn destination_page_index(&self, destination: &Destination) -> PdfResult<u32> {
        let id = self.validate_handle(&destination.page)?;
        let index = self
            .collect_page_entries()?
            .iter()
            .position(|p| p.page_id == id)
            .ok_or_else(|| PdfError::DocumentError("destination page is missing".into()))?;
        u32::try_from(index).map_err(|_| PdfError::ResourceLimit("destination page index".into()))
    }

    pub(crate) fn encode_destination(&self, dest: &Destination) -> PdfResult<PdfObject> {
        self.destination_page_index(dest)?;
        let (kind, args): (&str, Vec<Option<f32>>) = match dest.view {
            DestinationView::Xyz { left, top, zoom } => {
                if zoom.is_some_and(|v| v < 0.0) {
                    return Err(PdfError::DocumentError("negative destination zoom".into()));
                }
                ("XYZ", vec![left, top, zoom])
            }
            DestinationView::Fit => ("Fit", vec![]),
            DestinationView::FitH(v) => ("FitH", vec![v]),
            DestinationView::FitV(v) => ("FitV", vec![v]),
            DestinationView::FitB => ("FitB", vec![]),
            DestinationView::FitBH(v) => ("FitBH", vec![v]),
            DestinationView::FitBV(v) => ("FitBV", vec![v]),
            DestinationView::FitR {
                left,
                bottom,
                right,
                top,
            } => {
                if left >= right || bottom >= top {
                    return Err(PdfError::DocumentError(
                        "empty destination rectangle".into(),
                    ));
                }
                (
                    "FitR",
                    vec![Some(left), Some(bottom), Some(right), Some(top)],
                )
            }
        };
        let mut array = PdfArray::default();
        array.try_add_obj(PdfObject::PdfReference(PdfReference::from_id(
            dest.page.object_id(),
        )))?;
        array.try_add_obj(PdfObject::PdfName(PdfName::new(kind.into())))?;
        for value in args {
            array.try_add_obj(match value {
                Some(v) if v.is_finite() => PdfObject::PdfNumber(PdfNumber::new(v)),
                Some(_) => {
                    return Err(PdfError::DocumentError(
                        "non-finite destination coordinate".into(),
                    ));
                }
                None => PdfObject::PdfNull,
            })?;
        }
        Ok(PdfObject::PdfArray(array))
    }

    pub(crate) fn decode_destination(&self, object: &PdfObject) -> PdfResult<Destination> {
        let resolved = self.ctx().resolve(object)?;
        let resolved = if let Some(dict) = resolved.as_dict() {
            if dict
                .get("S")
                .is_some_and(|s| s.as_name().map(PdfName::name) != Some("GoTo"))
            {
                return Err(PdfError::UnsupportedFeature(
                    "destination action is not GoTo".into(),
                ));
            }
            self.ctx().resolve(
                dict.get("D")
                    .ok_or_else(|| PdfError::DocumentError("destination D missing".into()))?,
            )?
        } else {
            resolved
        };
        let array = resolved
            .as_array()
            .ok_or_else(|| PdfError::DocumentError("destination must be an array".into()))?;
        let id = array
            .get(0)
            .and_then(PdfObject::as_reference)
            .map(PdfReference::id)
            .ok_or_else(|| PdfError::DocumentError("destination page must be indirect".into()))?;
        let kind = self.ctx().resolve(
            array
                .get(1)
                .ok_or_else(|| PdfError::DocumentError("destination view missing".into()))?,
        )?;
        let kind = kind
            .as_name()
            .map(PdfName::name)
            .ok_or_else(|| PdfError::DocumentError("destination view must be a name".into()))?;
        let expected = match kind {
            "XYZ" => 5,
            "Fit" | "FitB" => 2,
            "FitH" | "FitV" | "FitBH" | "FitBV" => 3,
            "FitR" => 6,
            _ => {
                return Err(PdfError::UnsupportedFeature(format!(
                    "destination view {kind}"
                )));
            }
        };
        if array.len() != expected {
            return Err(PdfError::DocumentError(
                "destination parameter count".into(),
            ));
        }
        let number = |i: usize| -> PdfResult<Option<f32>> {
            let value = self.ctx().resolve(array.get(i).ok_or_else(|| {
                PdfError::DocumentError("destination coordinate missing".into())
            })?)?;
            match value {
                PdfObject::PdfNull => Ok(None),
                PdfObject::PdfNumber(n) if n.value().is_finite() => Ok(Some(n.value())),
                _ => Err(PdfError::DocumentError(
                    "destination coordinate must be finite or null".into(),
                )),
            }
        };
        let required = |i| {
            number(i)?
                .ok_or_else(|| PdfError::DocumentError("FitR coordinate cannot be null".into()))
        };
        let view = match kind {
            "XYZ" => DestinationView::Xyz {
                left: number(2)?,
                top: number(3)?,
                zoom: number(4)?,
            },
            "Fit" => DestinationView::Fit,
            "FitB" => DestinationView::FitB,
            "FitH" => DestinationView::FitH(number(2)?),
            "FitV" => DestinationView::FitV(number(2)?),
            "FitBH" => DestinationView::FitBH(number(2)?),
            "FitBV" => DestinationView::FitBV(number(2)?),
            "FitR" => DestinationView::FitR {
                left: required(2)?,
                bottom: required(3)?,
                right: required(4)?,
                top: required(5)?,
            },
            _ => unreachable!(),
        };
        let dest = Destination {
            page: self.object_handle(id)?,
            view,
        };
        self.encode_destination(&dest)?;
        Ok(dest)
    }
}
