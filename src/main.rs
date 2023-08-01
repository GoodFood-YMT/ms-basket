use actix_web::{web, App, HttpRequest, HttpResponse, HttpServer, Responder, middleware::Logger};
use redis::{Client, Commands, RedisError};
use serde::{Deserialize, Serialize};
use std::env;

const CATALOG_SERVICE_URL: &str = "http://ms-catalog.goodfood.svc.cluster.local/catalog/product";
// const CATALOG_SERVICE_URL: &str = "http://goodfood.localdev.me/catalog/product";

#[derive(Debug, Serialize, Deserialize)]
struct BasketItem {
    id: String,
    quantity: i32,
    label: String,
    description: String,
    price: f32,
    categoryId: String,
    restaurantId: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct RequestItem {
    id: String,
    quantity: i32,
}


#[derive(Debug, Serialize, Deserialize)]
struct Basket {
    user_id: String,
    items: Vec<BasketItem>,
}

#[derive(Debug, Serialize, Deserialize)]
struct ProductResponse {
    id: String,
    label: String,
    description: String,
    price: f32,
    visible: bool,
    quantity: i32,
    categoryId: String,
    restaurantId: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct ErrorResponse {
    error: String,
    message: String,
}

async fn fetch_product(product_id: &str) -> Result<ProductResponse, Box<dyn std::error::Error>> {
    let url = format!("{}/{}", CATALOG_SERVICE_URL, product_id);
    let client = reqwest::Client::new();

    let body = client.get(&url).send().await?.json::<ProductResponse>().await?;
    Ok(body)
}

fn get_user_id<'a>(req: &'a HttpRequest) -> Option<&'a str> {
    req.headers().get("UserID")?.to_str().ok()
}

async fn add_item_to_basket(
    req: HttpRequest,
    redis_client: web::Data<Client>,
    web::Json(item): web::Json<RequestItem>,
) -> impl Responder {
    if let Some(user_id) = get_user_id(&req) {
        let mut conn = redis_client.get_connection().unwrap();
        let basket_key = format!("basket:{}", user_id);
        let basket_exists: bool = conn.exists(&basket_key).unwrap();

        if !basket_exists {
            let product = fetch_product(&item.id).await;

            if let Ok(product) = product {
                let basket_item = BasketItem {
                    id: product.id,
                    quantity: item.quantity,
                    label: product.label,
                    description: product.description,
                    price: product.price,
                    categoryId: product.categoryId,
                    restaurantId: product.restaurantId,
                };

                let basket = Basket {
                    user_id: (*user_id).to_string(),
                    items: vec![basket_item],
                };

                let serialized_basket = serde_json::to_string(&basket).unwrap();
                let _: () = conn.set(&basket_key, serialized_basket).unwrap();

                HttpResponse::Ok().json(basket)
            } else {
                return HttpResponse::BadRequest().json(ErrorResponse {
                    error: "PRODUCT_NOT_FOUND".to_string(),
                    message: "Product not found".to_string(),
                });
            }
        } else {
            let serialized_basket: String = conn.get(&basket_key).unwrap();

            let mut basket: Basket = serde_json::from_str(&serialized_basket).unwrap();
            let mut item_index: Option<usize> = None;

            for (index, basket_item) in basket.items.iter().enumerate() {
                if basket_item.id == item.id {
                    item_index = Some(index);
                    break;
                }
            }

            if let Some(index) = item_index {
                basket.items[index].quantity += item.quantity;
            } else {
                let product = fetch_product(&item.id).await;

                if let Ok(product) = product {
                    // check if all items are from the same restaurant
                    if basket.items.len() > 0 {
                        // get first item in the basket and take the restaurantId
                        let restaurant_id = &basket.items[0].restaurantId;

                        // check if the restaurantId is the same as the one we are trying to add
                        if restaurant_id != &product.restaurantId {
                            return HttpResponse::BadRequest().json(ErrorResponse {
                                error: "NOT_SAME_RESTAURANT".to_string(),
                                message: "All items in the basket must be from the same restaurant".to_string(),
                            });
                        }
                    }
    
                    let basket_item = BasketItem {
                        id: product.id,
                        quantity: item.quantity,
                        label: product.label,
                        description: product.description,
                        price: product.price,
                        categoryId: product.categoryId,
                        restaurantId: product.restaurantId,
                    };

                    basket.items.push(basket_item);
                } else {
                    return HttpResponse::BadRequest().json(ErrorResponse {
                        error: "PRODUCT_NOT_FOUND".to_string(),
                        message: "Product not found".to_string(),
                    });
                }
            }

            let updated_serialized_basket = serde_json::to_string(&basket).unwrap();
            let _: () = conn.set(&basket_key, updated_serialized_basket).unwrap();

            HttpResponse::Ok().json(basket)
        }
    } else {
        return HttpResponse::BadRequest().json(ErrorResponse {
            error: "UNAUTHORIZED".to_string(),
            message: "Unauthorized".to_string(),
        });
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
        return HttpResponse::BadRequest().json(ErrorResponse {
            error: "UNAUTHORIZED".to_string(),
            message: "Unauthorized".to_string(),
        });
    }
    
}

async fn remove_item_from_basket(
    req: HttpRequest,
    redis_client: web::Data<Client>,
    item: web::Json<RequestItem>,
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
                if basket_item.id == item.id {
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
        return HttpResponse::BadRequest().json(ErrorResponse {
            error: "UNAUTHORIZED".to_string(),
            message: "Unauthorized".to_string(),
        });
    }
}

async fn clear_basket (req: HttpRequest, redis_client: web::Data<Client>) -> impl Responder {
    if let Some(user_id) = get_user_id(&req) {
        let mut conn = redis_client.get_connection().unwrap();
        let basket_key = format!("basket:{}", user_id);
        let basket_exists: bool = conn.exists(&basket_key).unwrap();

        if basket_exists {
            let serialized_basket: String = conn.get(&basket_key).unwrap();
            let mut basket: Basket = serde_json::from_str(&serialized_basket).unwrap();
            basket.items.clear();
            let updated_serialized_basket = serde_json::to_string(&basket).unwrap();
            let _: () = conn.set(&basket_key, updated_serialized_basket).unwrap();
        }

         HttpResponse::Ok().body("Basket cleared")
    } else {
        return HttpResponse::BadRequest().json(ErrorResponse {
            error: "UNAUTHORIZED".to_string(),
            message: "Unauthorized".to_string(),
        });
    }
}

#[actix_web::main]
async fn main() -> Result<(), RedisError> {
    let redis_host = env::var("REDIS_HOST").unwrap_or("127.0.0.1".to_string());
    let redis_port = env::var("REDIS_PORT").unwrap_or("6379".to_string());

    let redis_url = format!("redis://{}:{}/", redis_host, redis_port);
    let redis_client = Client::open(redis_url)?;

    env_logger::init_from_env(env_logger::Env::new().default_filter_or("info"));

    HttpServer::new(move || {
        App::new()
            .data(redis_client.clone())
            .wrap(Logger::default())
            .service(web::resource("/basket")
                .route(web::get().to(get_basket))
                .route(web::post().to(add_item_to_basket))
                .route(web::delete().to(remove_item_from_basket))
            )
            .service(web::resource("/basket/clear")
                .route(web::delete().to(clear_basket))
            )
            .service(web::resource("/basket/")
                .route(web::get().to(get_basket))
                .route(web::post().to(add_item_to_basket))
                .route(web::delete().to(remove_item_from_basket))
            )
            .service(web::resource("/basket/clear/")
                .route(web::delete().to(clear_basket))
            )
    })
    .bind("0.0.0.0:8080")?
    .run()
    .await?;
    
    Ok(())
}    
