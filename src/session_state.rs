use std::rc::Rc;

use actix_session::{Session, SessionExt, SessionGetError, SessionInsertError};
use actix_web::{dev::{forward_ready, Service, ServiceRequest, ServiceResponse, Transform}, FromRequest, HttpMessage};
use futures_util::{future::{ready, Either, LocalBoxFuture, Ready}, FutureExt};
use tracing::Instrument;
use uuid::Uuid;

use crate::utils::{e500, see_other};

pub struct TypedSession(Session);

impl TypedSession {
    const USER_ID_KEY: &'static str = "user_id";

    pub fn renew(&self) {
        self.0.renew();
    }
    
    pub fn insert_user_id(&self, user_id: Uuid) -> Result<(), SessionInsertError> {
        self.0.insert(Self::USER_ID_KEY, user_id)
    }

    pub fn get_user_id(&self) -> Result<Option<Uuid>, SessionGetError> {
        self.0.get(Self::USER_ID_KEY)
    }

    pub fn log_out(&self){
        self.0.purge()
    }
}

#[derive(Copy, Clone, Debug)]
pub struct UserId(Uuid);

impl std::fmt::Display for UserId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl std::ops::Deref for UserId {
    type Target = Uuid;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl FromRequest for TypedSession {
    type Error = <Session as FromRequest>::Error;
    type Future = Ready<Result<TypedSession, Self::Error>>;

    fn from_request(req: &actix_web::HttpRequest, _payload: &mut actix_web::dev::Payload) -> Self::Future {
        ready(Ok(TypedSession(req.get_session())))
    }
}

pub struct SessionAuthMiddlewareFactory;

impl SessionAuthMiddlewareFactory {
    pub fn default() -> Self{
        return SessionAuthMiddlewareFactory
    }
}

impl<S> Transform<S, ServiceRequest> for SessionAuthMiddlewareFactory
where 
    S: Service<ServiceRequest, Response = ServiceResponse, Error = actix_web::Error> + 'static
{
    type Response = S::Response;
    type Error = S::Error;
    type InitError = ();
    type Transform = SessionAuthMiddleware<S>;
    type Future = Ready<Result<Self::Transform, Self::InitError>>;


    fn new_transform(&self, service: S) -> Self::Future {
        let service = Rc::new(service);
        ready(Ok(SessionAuthMiddleware{ service }))
    }
}

pub struct SessionAuthMiddleware<S>{
    service: Rc<S>
}

impl<S> Service<ServiceRequest> for SessionAuthMiddleware<S>
where 
    S: Service<ServiceRequest, Response = ServiceResponse, Error = actix_web::Error> + 'static,

{
    type Response = S::Response;
    type Error = actix_web::Error;
    type Future = LocalBoxFuture<'static, Result<Self::Response, Self::Error>>;

    forward_ready!(service);

    #[tracing::instrument(
        "Checking if user is authenticated to access service",
        skip(self, req)
    )]
    fn call(&self, req: ServiceRequest) -> Self::Future {
        let session = TypedSession(req.get_session());

        let temp = session.get_user_id().map_err(e500);
        let either = match temp{
            Ok(r) => {
                if r.is_none(){
                    Either::Left(req.into_response(see_other("/login"))) 
                } else {
                    req.extensions_mut().insert(UserId(r.unwrap()));
                    Either::Right(self.service.call(req))
                }
            },

            Err(e) => return Box::pin(ready(Err(e)))
        };

        let current_span = tracing::Span::current();
        async move {
            let a = match either {
                Either::Left(res) => Ok(res),
                Either::Right(fut) => fut.await,
            };
            a
        }.instrument(current_span).boxed_local()
    }
}
