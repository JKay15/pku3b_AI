from pydantic import BaseModel

class entry_summary_Input(BaseModel):
    course: str
    entry: str
