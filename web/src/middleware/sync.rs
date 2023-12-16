use core::{cell::RefCell, mem};

use std::sync::mpsc::{sync_channel, Receiver};

use tokio::sync::mpsc::{unbounded_channel, UnboundedSender};

use crate::{
    body::RequestBody,
    context::WebContext,
    dev::service::{ready::ReadyService, Service},
    http::{Request, RequestExt, Response, WebResponse},
};

/// experimental type for sync function as middleware.
pub struct SyncMiddleware<F>(F);

impl<F> SyncMiddleware<F> {
    /// *. Sync middleware does not have access to request/response body.
    ///
    /// construct a new middleware with given sync function.
    /// the function must be actively calling [Next::call] and finish it to drive inner services to completion.
    /// panic in sync function middleware would result in a panic at task level and it's client connection would
    /// be terminated immediately.
    pub fn new<C, E>(func: F) -> Self
    where
        F: Fn(&mut Next<E>, WebContext<'_, C>) -> Result<Response<()>, E> + Send + Sync + 'static,
        C: Clone + Send + 'static,
        E: Send + 'static,
    {
        Self(func)
    }
}

pub struct Next<E> {
    tx: UnboundedSender<Request<RequestExt<()>>>,
    rx: Receiver<Result<Response<()>, E>>,
}

impl<E> Next<E> {
    pub fn call<C>(&mut self, mut ctx: WebContext<'_, C>) -> Result<Response<()>, E> {
        let req = mem::take(ctx.req_mut());
        self.tx.send(req).unwrap();
        self.rx.recv().unwrap()
    }
}

impl<F, S, E> Service<Result<S, E>> for SyncMiddleware<F>
where
    F: Clone,
{
    type Response = SyncService<F, S>;
    type Error = E;

    async fn call(&self, res: Result<S, E>) -> Result<Self::Response, Self::Error> {
        res.map(|service| SyncService {
            func: self.0.clone(),
            service,
        })
    }
}

pub struct SyncService<F, S> {
    func: F,
    service: S,
}

impl<'r, F, C, S, B, ResB, Err> Service<WebContext<'r, C, B>> for SyncService<F, S>
where
    F: Fn(&mut Next<Err>, WebContext<'_, C>) -> Result<Response<()>, Err> + Send + Clone + 'static,
    C: Clone + Send + 'static,
    S: for<'r2> Service<WebContext<'r, C, B>, Response = WebResponse<ResB>, Error = Err>,
    Err: Send + 'static,
{
    type Response = WebResponse<ResB>;
    type Error = Err;

    async fn call(&self, mut ctx: WebContext<'r, C, B>) -> Result<Self::Response, Self::Error> {
        let func = self.func.clone();
        let state = ctx.state().clone();
        let mut req = mem::take(ctx.req_mut());

        let (tx, mut rx) = unbounded_channel();
        let (tx2, rx2) = sync_channel(1);

        let mut next = Next { tx, rx: rx2 };
        let handle = tokio::task::spawn_blocking(move || {
            let mut body = RefCell::new(RequestBody::None);
            let ctx = WebContext::new(&mut req, &mut body, &state);
            func(&mut next, ctx)
        });

        *ctx.req_mut() = match rx.recv().await {
            Some(req) => req,
            None => {
                // tx is dropped which means spawned thread exited already. join it and panic if necessary.
                match handle.await.unwrap() {
                    Ok(_) => todo!("there is no support for body type yet"),
                    Err(e) => return Err(e),
                }
            }
        };

        match self.service.call(ctx).await {
            Ok(res) => {
                let (parts, body) = res.into_parts();
                tx2.send(Ok(Response::from_parts(parts, ()))).unwrap();
                let res = handle.await.unwrap()?;
                Ok(res.map(|_| body))
            }
            Err(e) => {
                tx2.send(Err(e)).unwrap();
                let res = handle.await.unwrap()?;
                Ok(res.map(|_| todo!("there is no support for body type yet")))
            }
        }
    }
}

impl<F, S> ReadyService for SyncService<F, S>
where
    S: ReadyService,
{
    type Ready = S::Ready;

    #[inline]
    async fn ready(&self) -> Self::Ready {
        self.service.ready().await
    }
}

#[cfg(test)]
mod test {
    use core::convert::Infallible;

    use crate::{bytes::Bytes, dev::service::fn_service, http::StatusCode, App};

    use super::*;

    async fn handler(req: WebContext<'_, &'static str>) -> Result<WebResponse, Infallible> {
        assert_eq!(*req.state(), "996");
        Ok(req.into_response(Bytes::new()))
    }

    fn middleware<E>(next: &mut Next<E>, ctx: WebContext<'_, &'static str>) -> Result<Response<()>, E> {
        next.call(ctx)
    }

    #[tokio::test]
    async fn sync_middleware() {
        let res = App::with_state("996")
            .at("/", fn_service(handler))
            .enclosed(SyncMiddleware::new(middleware))
            .finish()
            .call(())
            .await
            .unwrap()
            .call(Request::default())
            .await
            .unwrap();

        assert_eq!(res.status(), StatusCode::OK);
    }
}
