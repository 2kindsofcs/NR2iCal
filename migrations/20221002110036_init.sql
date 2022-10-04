-- Add migration script here
CREATE TABLE IF NOT EXISTS reservation (
    `id` int primary key not null,
    `business_id` int not null,
    `business_name` text not null,
    `item_id` int not null,
    `item_name` text not null,
    `options` text not null, 
    `start_date_time` datetime not null,
    `end_date_time` datetime not null,
    `location` text
);

