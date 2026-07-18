import type { Pet } from "@nimora/schemas";

export function petStatusMessage(pet: Pick<Pet, "state" | "energy" | "mood" | "satiety" | "cleanliness">): string {
  switch (pet.state) {
    case "sleeping": return "正在安静恢复体力";
    case "walking": return "去桌面上走走看看";
    case "working": return "正在专心陪你工作";
    case "dragged": return "抓稳啦…";
    case "interacting": return "很开心和你互动";
    default:
      if (pet.energy <= 25) return "有点困了，想休息一下";
      if (pet.satiety <= 25) return "肚子有点空，陪我吃点东西吧";
      if (pet.cleanliness <= 25) return "想整理一下，保持清清爽爽";
      if (pet.mood <= 25) return "今天想和你待一会儿";
      return "本地陪伴中";
  }
}
