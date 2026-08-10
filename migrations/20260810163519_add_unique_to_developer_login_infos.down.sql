-- Add down migration script here

DROP INDEX developer_login_info_email_lower_idx ON developer_login_info;
