use crate::geom::{matrix::Matrix, point::Point};

#[derive(Debug, Clone)]
pub struct Bezier {
    p1: Point,
    p2: Point,
    p3: Point,
}

impl Bezier {
    pub fn new(p1: Point, p2: Point, p3: Point) -> Self {
        Bezier { p1, p2, p3 }
    }
    pub fn p1(&self) -> &Point {
        &self.p1
    }
    pub fn p2(&self) -> &Point {
        &self.p2
    }
    pub fn p3(&self) -> &Point {
        &self.p3
    }
}

#[derive(Debug, Clone)]
pub enum Segment {
    MoveTo(Point),
    LineTo(Point),
    CurveTo(Bezier),
}
impl Segment {
    pub fn transform(&self, matrix: &Matrix) -> Segment {
        match self {
            Segment::MoveTo(p) => {
                let np = matrix.transform(p);
                Segment::MoveTo(np)
            }
            Segment::LineTo(p) => {
                let np = matrix.transform(p);
                Segment::LineTo(np)
            }
            Segment::CurveTo(bezier) => {
                let p1 = matrix.transform(bezier.p1());
                let p2 = matrix.transform(bezier.p2());
                let p3 = matrix.transform(bezier.p3());
                Segment::CurveTo(Bezier::new(p1, p2, p3))
            }
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct SubPath {
    segments: Vec<Segment>,
    closed: bool,
}

impl SubPath {
    pub fn segments(&self) -> &[Segment] {
        &self.segments
    }

    pub fn new(segments: Vec<Segment>, closed: bool) -> Self {
        SubPath { segments, closed }
    }

    pub fn add_segment(&mut self, segment: Segment) {
        self.segments.push(segment)
    }

    pub fn last_segment(&self) -> Option<&Segment> {
        self.segments.last()
    }

    pub fn last_mut_segment(&mut self) -> Option<&mut Segment> {
        self.segments.last_mut()
    }

    pub fn is_empty(&self) -> bool {
        self.segments.is_empty()
    }

    pub fn is_closed(&self) -> bool {
        self.closed
    }
    pub fn last_seg(&self) -> Option<&Segment> {
        self.segments.last()
    }
    pub fn pop(&mut self) -> Option<Segment> {
        self.segments.pop()
    }
    pub fn close(&mut self) {
        self.closed = true;
    }

    pub fn remove_last_opened_move(&mut self) {
        if self.closed {
            return;
        }
        if let Some(last) = self.segments.last() {
            match last {
                Segment::MoveTo(_) => {
                    self.segments.pop();
                }
                _ => {}
            }
        }
    }
    pub fn transform(&self, matrix: &Matrix) -> SubPath {
        let mut segments = Vec::new();
        for seg in self.segments.iter() {
            let new_seg = seg.transform(matrix);
            segments.push(new_seg)
        }
        SubPath::new(segments, self.closed)
    }
}

#[derive(Debug, Clone, Default)]
pub struct PdfPath {
    subpaths: Vec<SubPath>,
}
impl PdfPath {
    pub fn is_empty(&self) -> bool {
        self.subpaths.is_empty()
    }

    pub fn close_all(&mut self) {
        for path in self.subpaths.iter_mut() {
            path.close()
        }
    }

    pub fn add_subpath(&mut self, subpath: SubPath) {
        self.subpaths.push(subpath)
    }

    pub fn last_mut_subpath(&mut self) -> Option<&mut SubPath> {
        self.subpaths.last_mut()
    }

    pub fn subpaths(&self) -> &[SubPath] {
        self.subpaths.as_slice()
    }
}
