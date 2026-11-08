import { revalidateTag } from "next/cache";
import { isAuthorized } from "@/lib/revalidate";

export async function POST(request: Request): Promise<Response> {
  const secret = process.env.REVALIDATE_SECRET;
  if (!secret) {
    return new Response("Revalidation is not configured", { status: 503 });
  }
  if (!isAuthorized(request.headers.get("authorization"), secret)) {
    return new Response("Unauthorized", { status: 401 });
  }
  revalidateTag("releases", { expire: 0 });
  return new Response(null, { status: 204 });
}
