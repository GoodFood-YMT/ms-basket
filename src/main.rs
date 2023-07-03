use actix_web::{web, App, HttpRequest, HttpResponse, HttpServer, Responder};
use redis::{Client, Commands, RedisError};
use serde::{Deserialize, Serialize};
use std::env;

#[derive(Debug, Serialize, Deserialize)]
struct BasketItem {
    product_id: i32,
    quantity: i32,
}

#[derive(Debug, Serialize, Deserialize)]
struct Basket {
    user_id: String,
    items: Vec<BasketItem>,
}

fn get_user_id<'a>(req: &'a HttpRequest) -> Option<&'a str> {
    req.headers().get("UserID")?.to_str().ok()
}

async fn add_item_to_basket(
    req: HttpRequest,
    redis_client: web::Data<Client>,
    web::Json(item): web::Json<BasketItem>,
) -> impl Responder {
    if let Some(user_id) = get_user_id(&req) {
        let mut conn = redis_client.get_connection().unwrap();
        let basket_key = format!("basket:{}", user_id);
        let basket_exists: bool = conn.exists(&basket_key).unwrap();

        if !basket_exists {
            let basket = Basket {
                user_id: (*user_id).to_string(),
                items: vec![item],
            };

            let serialized_basket = serde_json::to_string(&basket).unwrap();
            let _: () = conn.set(&basket_key, serialized_basket).unwrap();

            HttpResponse::Ok().json(basket)
        } else {
            let serialized_basket: String = conn.get(&basket_key).unwrap();

            let mut basket: Basket = serde_json::from_str(&serialized_basket).unwrap();
            let mut item_index: Option<usize> = None;

            for (index, basket_item) in basket.items.iter().enumerate() {
                if basket_item.product_id == item.product_id {
                    item_index = Some(index);
                    break;
                }
            }

            if let Some(index) = item_index {
                basket.items[index].quantity += item.quantity;
            } else {
                basket.items.push(item);
            }

            let updated_serialized_basket = serde_json::to_string(&basket).unwrap();
            let _: () = conn.set(&basket_key, updated_serialized_basket).unwrap();

            HttpResponse::Ok().json(basket)
        }
    } else {
        HttpResponse::BadRequest().body("UserID header is required")
    }
}

async fn get_basket(req: HttpRequest, redis_client: web::Data<Client>) -> impl Responder {
    if let Some(user_id) = get_user_id(&req) {
        let mut conn = redis_client.get_connection().unwrap();
        let basket_key = format!("basket:{}", user_id);
        let basket_exists: bool = conn.exists(&basket_key).unwrap();

        if !basket_exists {
            HttpResponse::Ok().json({
                Basket {
                    user_id: (*user_id).to_string(),
                    items: vec![],
                }
            })
        } else {
            let serialized_basket: String = conn.get(&basket_key).unwrap();
            let basket: Basket = serde_json::from_str(&serialized_basket).unwrap();

            HttpResponse::Ok().json(basket)
        }
    } else {
        HttpResponse::BadRequest().body("UserID header is required")
    }
    
}

async fn remove_item_from_basket(
    req: HttpRequest,
    redis_client: web::Data<Client>,
    item: web::Json<BasketItem>,
) -> impl Responder {
    if let Some(user_id) = get_user_id(&req) {
        let mut conn = redis_client.get_connection().unwrap();
        let basket_key = format!("basket:{}", user_id);
        let basket_exists: bool = conn.exists(&basket_key).unwrap();

        if basket_exists {
            let serialized_basket: String = conn.get(&basket_key).unwrap();
            let mut basket: Basket = serde_json::from_str(&serialized_basket).unwrap();
            let mut item_index: Option<usize> = None;

            for (index, basket_item) in basket.items.iter().enumerate() {
                if basket_item.product_id == item.product_id {
                    item_index = Some(index);
                    break;
                }
            }

            if let Some(index) = item_index {
                let quantity_to_remove = item.quantity;
                if basket.items[index].quantity > quantity_to_remove {
                    basket.items[index].quantity -= quantity_to_remove;
                    let updated_serialized_basket = serde_json::to_string(&basket).unwrap();
                    let _: () = conn.set(&basket_key, updated_serialized_basket).unwrap();

                    HttpResponse::Ok().json(basket)
                } else {
                    basket.items.remove(index);
                    let updated_serialized_basket = serde_json::to_string(&basket).unwrap();
                    let _: () = conn.set(&basket_key, updated_serialized_basket).unwrap();

                    HttpResponse::Ok().json(basket)
                }
            } else {
                HttpResponse::Ok().json(basket)
            }
        } else {
            HttpResponse::Ok().json({
                Basket {
                    user_id: (*user_id).to_string(),
                    items: vec![],
                }
            })
        }
    } else {
        return HttpResponse::BadRequest().body("UserID header is required");
    }
}

#[actix_web::main]
async fn main() -> Result<(), RedisError> {
    let redis_host = env::var("REDIS_HOST").unwrap_or("127.0.0.1".to_string());
    let redis_port = env::var("REDIS_PORT").unwrap_or("6379".to_string());

    let redis_url = format!("redis://{}:{}/", redis_host, redis_port);
    let redis_client = Client::open(redis_url)?;

    HttpServer::new(move || {
        App::new()
            .data(redis_client.clone())
            .service(web::resource("/")
                .route(web::get().to(get_basket))
                .route(web::post().to(add_item_to_basket))
                .route(web::delete().to(remove_item_from_basket))
            )
    })
    .bind("127.0.0.1:8080")?
    .run()
    .await?;
    
    Ok(())
}    
