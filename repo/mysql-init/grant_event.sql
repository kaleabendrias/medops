-- Grant EVENT privilege to app_user so SQLx migrations can create scheduled events.
GRANT EVENT ON hospital_platform.* TO 'app_user'@'%';
FLUSH PRIVILEGES;
