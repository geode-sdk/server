-- Add up migration script here

CREATE UNIQUE INDEX developer_login_info_email_lower_idx
ON developer_login_info(LOWER(email));
