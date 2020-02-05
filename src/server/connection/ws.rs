use actix_web::HttpResponse;

pub fn handle() -> HttpResponse {
    HttpResponse::Ok().finish()
}
