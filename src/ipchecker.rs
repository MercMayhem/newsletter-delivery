use std::{future::{ready, Ready}, collections::HashSet};

use actix_web::{dev::{Transform, Service, ServiceRequest, ServiceResponse, forward_ready}, Error, HttpResponse};
use futures_util::future::{LocalBoxFuture, Either};


pub struct IpChecker {
    pub allows: HashSet<String>
}

impl IpChecker {
    pub fn allow(mut self, ip: &str) -> Self {
        self.allows.insert(ip.to_string());
        self
    }
}

impl Default for IpChecker {
    fn default() -> Self {
        Self { allows: HashSet::new() }
    }
}

impl<S> Transform<S, ServiceRequest> for IpChecker
where
    S: Service<ServiceRequest, Response = ServiceResponse, Error = Error>,
    S::Future: 'static,
{
    type Response = S::Response;
    type Error = S::Error;
    type InitError = ();
    type Transform = IpCheckerMiddleware<S>;
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ready(Ok(IpCheckerMiddleware { service, allows: self.allows.clone() }))
    }
}

pub struct IpCheckerMiddleware<S> {
    service: S,
    allows: HashSet<String>
}

impl<S> Service<ServiceRequest> for IpCheckerMiddleware<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse, Error = Error>,
    S::Future: 'static,
{
    type Response = S::Response;
    type Error = Error;
    type Future = LocalBoxFuture<'static, Result<Self::Response, Self::Error>>;

    forward_ready!(service);

    fn call(&self, req: ServiceRequest) -> Self::Future {
        let conn_info = req.connection_info().clone();
        let mut forbidden = true;
        if let Some(val) =  conn_info.realip_remote_addr() {
            println!("Real Address {:?}", val);
            if self.allows.contains(val) {
                forbidden = false
            }
        }

        let either = if forbidden {
            Either::Left(req.into_response(HttpResponse::Forbidden().body("Forbidden")))
        } else {
            Either::Right(self.service.call(req))
        };

        Box::pin(async move {
            let a = match either {
                Either::Left(res) => Ok(res),
                Either::Right(fut) => fut.await,
            };
            a
            // let res = fut.await?;
            // Ok(HttpResponse::Forbidden().finish())
            // Ok(res)
        })
    }
}
