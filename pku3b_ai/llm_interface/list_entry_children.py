from pydantic import BaseModel

class list_entry_children_Input(BaseModel):
    course: str
    entry: str
