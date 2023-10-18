use std::ops::Deref;

use crate::{
    body::BodyStream,
    handler::{error::ExtractError, FromRequest},
    request::WebRequest,
};

#[derive(Debug)]
pub struct PathRef<'a>(pub &'a str);

impl Deref for PathRef<'_> {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        self.0
    }
}

impl<'a, 'r, C, B> FromRequest<'a, WebRequest<'r, C, B>> for PathRef<'a>
where
    B: BodyStream,
{
    type Type<'b> = PathRef<'b>;
    type Error = ExtractError<B::Error>;

    #[inline]
    async fn from_request(req: &'a WebRequest<'r, C, B>) -> Result<Self, Self::Error> {
        Ok(PathRef(req.req().uri().path()))
    }
}
