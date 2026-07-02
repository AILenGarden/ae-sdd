package ai.nubase.auth.dto.request.admin;

import cn.hutool.json.JSONUtil;
import lombok.AllArgsConstructor;
import lombok.Data;
import lombok.NoArgsConstructor;

/**
 * Request DTO for executing SQL statements via Admin API.
 * Allows administrators to run DDL/DML operations directly on the database.
 *
 * WARNING: This is a powerful and potentially dangerous operation.
 * Should only be used with service_role authentication.
 */
@Data
@NoArgsConstructor
@AllArgsConstructor
public class ExecuteSqlRequest {

    /**
     * SQL query to execute (DDL or DML)
     * Examples:
     * - CREATE TABLE users_extended (id UUID PRIMARY KEY, ...)
     * - ALTER TABLE users ADD COLUMN nickname VARCHAR(100)
     * - INSERT INTO custom_table VALUES (...)
     */
    private String query;

    /**
     * Optional parameters for parameterized queries (prevents SQL injection)
     * Example: SELECT * FROM users WHERE email = ?
     * params: ["user@example.com"]
     */
//    private List<Object> params;
    public static void main(String[] args) {
        String sql = "\n" +
                "-- ============================================\n" +
                "-- Phase 1: Foundation (Utilities & ENUMs)\n" +
                "-- ============================================\n" +
                "\n" +
                "-- Standard updated_at trigger function\n" +
                "CREATE OR REPLACE FUNCTION update_updated_at_column()\n" +
                "RETURNS TRIGGER AS $$\n" +
                "BEGIN\n" +
                "    NEW.updated_at = now();\n" +
                "    RETURN NEW;\n" +
                "END;\n" +
                "$$ LANGUAGE plpgsql;\n" +
                "\n" +
                "-- Sales opportunity stage enum\n" +
                "CREATE TYPE opportunity_stage AS ENUM (\n" +
                "    'initial_contact',      -- Initial contact\n" +
                "    'requirement_analysis', -- Requirement analysis\n" +
                "    'proposal',            -- Proposal / quotation\n" +
                "    'negotiation',         -- Negotiation\n" +
                "    'closed_won',          -- Closed - won\n" +
                "    'closed_lost'          -- Closed - lost\n" +
                ");\n" +
                "\n" +
                "-- Communication channel enum\n" +
                "CREATE TYPE communication_type AS ENUM (\n" +
                "    'phone',    -- Phone\n" +
                "    'email',    -- Email\n" +
                "    'visit',    -- On-site visit\n" +
                "    'meeting'   -- Meeting\n" +
                ");\n" +
                "\n" +
                "-- ============================================\n" +
                "-- Phase 2: DDL (Tables with Logical FKs)\n" +
                "-- ============================================\n" +
                "\n" +
                "-- User roles table\n" +
                "CREATE TABLE public.user_roles (\n" +
                "    id UUID DEFAULT gen_random_uuid() PRIMARY KEY,\n" +
                "    user_id UUID NOT NULL,\n" +
                "    role TEXT NOT NULL CHECK (role IN ('admin', 'manager', 'employee')),\n" +
                "    created_at TIMESTAMPTZ DEFAULT now(),\n" +
                "    UNIQUE(user_id, role)\n" +
                ");\n" +
                "CREATE INDEX idx_user_roles_user_id ON public.user_roles(user_id);\n" +
                "COMMENT ON TABLE public.user_roles IS 'User roles table';\n" +
                "\n" +
                "-- Sales person profile table\n" +
                "CREATE TABLE public.sales_profiles (\n" +
                "    id UUID PRIMARY KEY,\n" +
                "    email TEXT,\n" +
                "    full_name TEXT NOT NULL,\n" +
                "    department TEXT,\n" +
                "    position TEXT,\n" +
                "    phone TEXT,\n" +
                "    avatar_url TEXT,\n" +
                "    created_at TIMESTAMPTZ DEFAULT now(),\n" +
                "    updated_at TIMESTAMPTZ DEFAULT now()\n" +
                ");\n" +
                "COMMENT ON TABLE public.sales_profiles IS 'Sales person profile table';\n" +
                "\n" +
                "-- Customer information table\n" +
                "CREATE TABLE public.customers (\n" +
                "    id UUID DEFAULT gen_random_uuid() PRIMARY KEY,\n" +
                "    company_name TEXT NOT NULL,\n" +
                "    contact_person TEXT,\n" +
                "    phone TEXT,\n" +
                "    email TEXT,\n" +
                "    address TEXT,\n" +
                "    industry TEXT,\n" +
                "    website TEXT,\n" +
                "    description TEXT,\n" +
                "    owner_id UUID NOT NULL,\n" +
                "    created_at TIMESTAMPTZ DEFAULT now(),\n" +
                "    updated_at TIMESTAMPTZ DEFAULT now()\n" +
                ");\n" +
                "CREATE INDEX idx_customers_owner_id ON public.customers(owner_id);\n" +
                "CREATE INDEX idx_customers_company_name ON public.customers(company_name);\n" +
                "CREATE INDEX idx_customers_industry ON public.customers(industry);\n" +
                "COMMENT ON TABLE public.customers IS 'Customer information table';\n" +
                "COMMENT ON COLUMN public.customers.owner_id IS 'Owner ID (logical FK to sales_profiles.id)';\n" +
                "\n" +
                "-- Sales opportunities table\n" +
                "CREATE TABLE public.opportunities (\n" +
                "    id UUID DEFAULT gen_random_uuid() PRIMARY KEY,\n" +
                "    name TEXT NOT NULL,\n" +
                "    customer_id UUID NOT NULL,\n" +
                "    owner_id UUID NOT NULL,\n" +
                "    amount DECIMAL(15, 2),\n" +
                "    stage opportunity_stage NOT NULL DEFAULT 'initial_contact',\n" +
                "    probability INTEGER CHECK (probability >= 0 AND probability <= 100),\n" +
                "    expected_close_date DATE,\n" +
                "    description TEXT,\n" +
                "    created_at TIMESTAMPTZ DEFAULT now(),\n" +
                "    updated_at TIMESTAMPTZ DEFAULT now()\n" +
                ");\n" +
                "CREATE INDEX idx_opportunities_customer_id ON public.opportunities(customer_id);\n" +
                "CREATE INDEX idx_opportunities_owner_id ON public.opportunities(owner_id);\n" +
                "CREATE INDEX idx_opportunities_stage ON public.opportunities(stage);\n" +
                "COMMENT ON TABLE public.opportunities IS 'Sales opportunities table';\n" +
                "COMMENT ON COLUMN public.opportunities.customer_id IS 'Customer ID (logical FK to customers.id)';\n" +
                "COMMENT ON COLUMN public.opportunities.owner_id IS 'Owner ID (logical FK to sales_profiles.id)';\n" +
                "\n" +
                "-- Communications log table\n" +
                "CREATE TABLE public.communications (\n" +
                "    id UUID DEFAULT gen_random_uuid() PRIMARY KEY,\n" +
                "    customer_id UUID NOT NULL,\n" +
                "    opportunity_id UUID,\n" +
                "    owner_id UUID NOT NULL,\n" +
                "    communication_type communication_type NOT NULL,\n" +
                "    subject TEXT NOT NULL,\n" +
                "    content TEXT,\n" +
                "    communication_date TIMESTAMPTZ NOT NULL DEFAULT now(),\n" +
                "    next_follow_up_date DATE,\n" +
                "    created_at TIMESTAMPTZ DEFAULT now(),\n" +
                "    updated_at TIMESTAMPTZ DEFAULT now()\n" +
                ");\n" +
                "CREATE INDEX idx_communications_customer_id ON public.communications(customer_id);\n" +
                "CREATE INDEX idx_communications_opportunity_id ON public.communications(opportunity_id);\n" +
                "CREATE INDEX idx_communications_owner_id ON public.communications(owner_id);\n" +
                "CREATE INDEX idx_communications_date ON public.communications(communication_date DESC);\n" +
                "COMMENT ON TABLE public.communications IS 'Communications log table';\n" +
                "COMMENT ON COLUMN public.communications.customer_id IS 'Customer ID (logical FK to customers.id)';\n" +
                "COMMENT ON COLUMN public.communications.opportunity_id IS 'Opportunity ID (logical FK to opportunities.id, optional)';\n" +
                "COMMENT ON COLUMN public.communications.owner_id IS 'Author ID (logical FK to sales_profiles.id)';\n" +
                "\n" +
                "-- ============================================\n" +
                "-- Phase 3: Logic (Table-Dependent Functions)\n" +
                "-- ============================================\n" +
                "\n" +
                "-- Role-check function\n" +
                "CREATE OR REPLACE FUNCTION public.has_role(_role TEXT)\n" +
                "RETURNS BOOLEAN\n" +
                "LANGUAGE sql\n" +
                "SECURITY DEFINER\n" +
                "SET search_path = public\n" +
                "STABLE\n" +
                "AS $$\n" +
                "  SELECT EXISTS (\n" +
                "    SELECT 1 FROM user_roles \n" +
                "    WHERE user_id = auth.uid() \n" +
                "    AND role = _role\n" +
                "  );\n" +
                "$$;\n" +
                "\n" +
                "-- Check whether the caller is admin or manager\n" +
                "CREATE OR REPLACE FUNCTION public.is_manager_or_admin()\n" +
                "RETURNS BOOLEAN\n" +
                "LANGUAGE sql\n" +
                "SECURITY DEFINER\n" +
                "SET search_path = public\n" +
                "STABLE\n" +
                "AS $$\n" +
                "  SELECT EXISTS (\n" +
                "    SELECT 1 FROM user_roles \n" +
                "    WHERE user_id = auth.uid() \n" +
                "    AND role IN ('admin', 'manager')\n" +
                "  );\n" +
                "$$;\n" +
                "\n" +
                "-- ============================================\n" +
                "-- Phase 4: Security (RLS Policies)\n" +
                "-- ============================================\n" +
                "\n" +
                "-- User roles RLS\n" +
                "ALTER TABLE public.user_roles ENABLE ROW LEVEL SECURITY;\n" +
                "\n" +
                "CREATE POLICY \"Admins can manage all roles\" ON public.user_roles\n" +
                "FOR ALL\n" +
                "USING (has_role('admin'));\n" +
                "\n" +
                "CREATE POLICY \"Users can view their own roles\" ON public.user_roles\n" +
                "FOR SELECT\n" +
                "USING (auth.uid() = user_id);\n" +
                "\n" +
                "-- Sales profiles RLS\n" +
                "ALTER TABLE public.sales_profiles ENABLE ROW LEVEL SECURITY;\n" +
                "\n" +
                "CREATE POLICY \"All authenticated users can view sales profiles\" ON public.sales_profiles\n" +
                "FOR SELECT\n" +
                "USING (auth.uid() IS NOT NULL);\n" +
                "\n" +
                "CREATE POLICY \"Users can update their own profile\" ON public.sales_profiles\n" +
                "FOR UPDATE\n" +
                "USING (auth.uid() = id);\n" +
                "\n" +
                "CREATE POLICY \"Admins can manage all profiles\" ON public.sales_profiles\n" +
                "FOR ALL\n" +
                "USING (has_role('admin'));\n" +
                "\n" +
                "-- Customers RLS\n" +
                "ALTER TABLE public.customers ENABLE ROW LEVEL SECURITY;\n" +
                "\n" +
                "CREATE POLICY \"Sales reps can view their own customers\" ON public.customers\n" +
                "FOR SELECT\n" +
                "USING (auth.uid() = owner_id OR is_manager_or_admin());\n" +
                "\n" +
                "CREATE POLICY \"Sales reps can create customers\" ON public.customers\n" +
                "FOR INSERT\n" +
                "WITH CHECK (auth.uid() = owner_id);\n" +
                "\n" +
                "CREATE POLICY \"Sales reps can update their own customers\" ON public.customers\n" +
                "FOR UPDATE\n" +
                "USING (auth.uid() = owner_id OR is_manager_or_admin());\n" +
                "\n" +
                "CREATE POLICY \"Admins can delete customers\" ON public.customers\n" +
                "FOR DELETE\n" +
                "USING (has_role('admin'));\n" +
                "\n" +
                "-- Opportunities RLS\n" +
                "ALTER TABLE public.opportunities ENABLE ROW LEVEL SECURITY;\n" +
                "\n" +
                "CREATE POLICY \"Sales reps can view their own opportunities\" ON public.opportunities\n" +
                "FOR SELECT\n" +
                "USING (auth.uid() = owner_id OR is_manager_or_admin());\n" +
                "\n" +
                "CREATE POLICY \"Sales reps can create opportunities\" ON public.opportunities\n" +
                "FOR INSERT\n" +
                "WITH CHECK (auth.uid() = owner_id);\n" +
                "\n" +
                "CREATE POLICY \"Sales reps can update their own opportunities\" ON public.opportunities\n" +
                "FOR UPDATE\n" +
                "USING (auth.uid() = owner_id OR is_manager_or_admin());\n" +
                "\n" +
                "CREATE POLICY \"Admins can delete opportunities\" ON public.opportunities\n" +
                "FOR DELETE\n" +
                "USING (has_role('admin'));\n" +
                "\n" +
                "-- Communications RLS\n" +
                "ALTER TABLE public.communications ENABLE ROW LEVEL SECURITY;\n" +
                "\n" +
                "CREATE POLICY \"Sales reps can view their own communications\" ON public.communications\n" +
                "FOR SELECT\n" +
                "USING (auth.uid() = owner_id OR is_manager_or_admin());\n" +
                "\n" +
                "CREATE POLICY \"Sales reps can create communications\" ON public.communications\n" +
                "FOR INSERT\n" +
                "WITH CHECK (auth.uid() = owner_id);\n" +
                "\n" +
                "CREATE POLICY \"Sales reps can update their own communications\" ON public.communications\n" +
                "FOR UPDATE\n" +
                "USING (auth.uid() = owner_id OR is_manager_or_admin());\n" +
                "\n" +
                "CREATE POLICY \"Admins can delete communications\" ON public.communications\n" +
                "FOR DELETE\n" +
                "USING (has_role('admin'));\n" +
                "\n" +
                "-- ============================================\n" +
                "-- Phase 5: Automation (Triggers)\n" +
                "-- ============================================\n" +
                "\n" +
                "-- updated_at auto-update triggers\n" +
                "CREATE TRIGGER update_sales_profiles_updated_at\n" +
                "    BEFORE UPDATE ON public.sales_profiles\n" +
                "    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();\n" +
                "\n" +
                "CREATE TRIGGER update_customers_updated_at\n" +
                "    BEFORE UPDATE ON public.customers\n" +
                "    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();\n" +
                "\n" +
                "CREATE TRIGGER update_opportunities_updated_at\n" +
                "    BEFORE UPDATE ON public.opportunities\n" +
                "    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();\n" +
                "\n" +
                "CREATE TRIGGER update_communications_updated_at\n" +
                "    BEFORE UPDATE ON public.communications\n" +
                "    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();\n" +
                "\n" +
                "-- Auto-create profile and role on new user signup\n" +
                "CREATE OR REPLACE FUNCTION public.handle_new_user()\n" +
                "RETURNS TRIGGER\n" +
                "LANGUAGE plpgsql\n" +
                "SECURITY DEFINER SET search_path = public\n" +
                "AS $$\n" +
                "BEGIN\n" +
                "    -- Create the sales profile\n" +
                "    INSERT INTO public.sales_profiles (id, email, full_name)\n" +
                "    VALUES (NEW.id, NEW.email, COALESCE(NEW.raw_user_meta_data->>'full_name', split_part(NEW.email, '@', 1)));\n" +
                "    \n" +
                "    -- Assign the default role\n" +
                "    INSERT INTO public.user_roles (user_id, role)\n" +
                "    VALUES (NEW.id, 'employee');\n" +
                "    \n" +
                "    RETURN NEW;\n" +
                "END;\n" +
                "$$;\n" +
                "\n" +
                "CREATE TRIGGER on_auth_user_created\n" +
                "    AFTER INSERT ON auth.users\n" +
                "    FOR EACH ROW EXECUTE FUNCTION public.handle_new_user();\n";
        System.out.println(sql);
        ExecuteSqlRequest request = new ExecuteSqlRequest();
        request.setQuery(sql);
        System.out.println(JSONUtil.toJsonStr(request));
    }
}
