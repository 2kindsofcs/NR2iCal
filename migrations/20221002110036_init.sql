-- Add migration script here
CREATE TABLE IF NOT EXISTS reservation (
    `id` int primary key,
    `name` text not null,
    `start_date` datetime not null,
    `end_date` datetime not null
) 