use crate::color::value::ColorValue;
use crate::error::{PdfError, PdfResult};
use crate::objects::PdfObject;

use super::value::ColorRgb;

#[derive(Debug, Clone)]
pub struct Lab {
    white_point: [f32; 3],
    black_point: [f32; 3],
    range: [f32; 4],
}

impl Default for Lab {
    fn default() -> Self {
        Lab {
            white_point: [1.0, 1.0, 1.0],
            black_point: [0.0, 0.0, 0.0],
            range: [-100.0, 100.0, -100.0, 100.0],
        }
    }
}

impl Lab {
    pub fn try_new(obj: &PdfObject) -> PdfResult<Self> {
        let mut color = Lab::default();
        if let Some(r) = obj.as_dict().unwrap().get("Range") {
            let ra = r.as_array().ok_or(PdfError::ColorError(format!(
                "Lab Range need an array got:{:?}",
                r
            )))?;
            if ra.len() != 4 {
                return Err(PdfError::ColorError(format!(
                    "Lab Range need 4 elements :{:?}",
                    ra
                )));
            }
            for i in 0..4 {
                color.range[i] =
                    ra.get(i)
                        .unwrap()
                        .as_f32()
                        .ok_or(PdfError::ColorError(format!(
                            "Lab Range element need number got:{:?}",
                            ra.get(i)
                        )))?;
            }
        }

        match obj.as_dict().unwrap().get("WhitePoint") {
            Some(wh) => {
                let wha = wh.as_array().ok_or(PdfError::ColorError(format!(
                    "Lab  WhitePoint is not an array got:{:?}",
                    obj
                )))?;
                if wha.len() != 3 {
                    return Err(PdfError::ColorError(format!(
                        "Lab WhitePoint need 3 elements :{:?}",
                        wha
                    )));
                }
                for i in 0..3 {
                    let v = wha
                        .get(i)
                        .unwrap()
                        .as_f32()
                        .ok_or(PdfError::ColorError(format!(
                            "Lab WhitePoint {:?} value is not an number:{:?}",
                            i,
                            wha.get(i)
                        )))?;
                    color.white_point[i] = v;
                }
            }
            None => {
                return Err(PdfError::ColorError(
                    "Lab WhitePoint is required".to_string(),
                ));
            }
        }
        if let Some(bp) = obj.as_dict().unwrap().get("BlackPoint") {
            let bpa = bp.as_array().ok_or(PdfError::ColorError(format!(
                "Lab BlackPoint is not an array got:{:?}",
                obj
            )))?;
            if bpa.len() != 3 {
                return Err(PdfError::ColorError(format!(
                    "Lab BlackPoint need 3 elements :{:?}",
                    bpa
                )));
            }
            for i in 0..3 {
                let v = bpa
                    .get(i)
                    .unwrap()
                    .as_f32()
                    .ok_or(PdfError::ColorError(format!(
                        "Lab BlackPoint {:?} value is not an number:{:?}",
                        i,
                        bpa.get(i)
                    )))?;
                color.black_point[i] = v;
            }
        }

        Ok(color)
    }

    pub fn default_value(&self) -> ColorValue {
        let mut default = vec![0.0, 0.0, 0.0];
        if self.range[0] > 0.0 {
            default[1] = self.range[0];
        } else if self.range[1] < 0.0 {
            default[1] = self.range[1];
        }

        if self.range[2] > 0.0 {
            default[2] = self.range[2];
        } else if self.range[3] < 0.0 {
            default[2] = self.range[3];
        }
        ColorValue::new(default)
    }

    pub fn number_of_components(&self) -> usize {
        3
    }
    pub fn rgb(&self, value: &ColorValue) -> PdfResult<ColorRgb> {
        unimplemented!()
    }

    pub fn range(&self) -> &[f32] {
        &self.range
    }
}
