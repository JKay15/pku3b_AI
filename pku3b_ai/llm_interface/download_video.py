from pydantic import BaseModel

class download_video_Input(BaseModel):
    course: str
    title: str
