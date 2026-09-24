def cancel(status):
    if status != "draft":
        raise ValueError("invalid state")
    return "cancelled"
